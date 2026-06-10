#!/usr/bin/env python3
"""
Generate WBS Excel report from AutoPASS IP 2.0 kanban data.

Usage:
    kanban --format json report wbs | python3 tools/kanban/scripts/wbs_report.py

    # explicit output path
    kanban --format json report wbs | python3 tools/kanban/scripts/wbs_report.py \
        --output delivery/reports/2026-06-07.001.autopass_ip_2.0_wbs_report.xlsx

Default output is delivery/reports/<date>.<nnn>.autopass_ip_2.0_wbs_report.xlsx
where <nnn> is a zero-padded sequence number that auto-increments each run.

The script reads JSON from stdin (produced by `kanban --format json report wbs`)
and writes an xlsx report with:
  - Hierarchical WBS numbering (phase.epic.story) rebuilt from live data
  - SUM formulas for story-point totals on epic and phase rows
  - Start Date and End Date columns: actual for done/in-progress stories,
    daily-throughput estimates for not-yet-started stories
  - Estimated hours derived from observed daily throughput for unstarted stories
  - Sprint burndown prognosis sheet
  - Phase summary sheet
  - Legend sheet
"""

import argparse
import json
import sys
from datetime import date, datetime, timedelta
from pathlib import Path

try:
    import openpyxl
    from openpyxl.styles import Alignment, Font, PatternFill
except ImportError:
    print("ERROR: openpyxl is required. Install with: pip3 install openpyxl", file=sys.stderr)
    sys.exit(1)


# ── Colour palette ────────────────────────────────────────────────────────────

COLOUR_TITLE_BG            = "FF0D1F40"
COLOUR_HEADER_BG           = "FF1A3060"
COLOUR_PHASE_BG            = "FF1F3864"
COLOUR_EPIC_BG             = "FF2E5EAA"
COLOUR_STORY_BG            = "FFFFFFFF"
COLOUR_STORY_INPROGRESS_BG = "FFE6D0FF"  # soft purple
COLOUR_STORY_DONE_BG       = "FFD0F0D0"  # soft green
COLOUR_WHITE_FG            = "FFFFFFFF"
COLOUR_DARK_FG             = "FF1F1F1F"

STATUS_MAP = {
    "draft":        "DRAFT",
    "ready":        "READY",
    "todo":         "TODO",
    "in-progress":  "IN PROGRESS",
    "ready-for-qa": "READY FOR QA",
    "blocked":      "BLOCKED",
    "done":         "DONE",
    "dropped":      "DROPPED",
}

PHASE_META = {
    "F1": {"title": "Phase 1 – Etablering (Establishment)",              "milestone": "MP1 – Foundation",            "period": "Q2 2026", "priority": "Critical"},
    "F2": {"title": "Phase 2 – Utvikling: Kjernelogikk (Core Logic)",    "milestone": "MP2 – Core Logic",            "period": "Q3 2026", "priority": "Critical"},
    "F3": {"title": "Phase 3 – Utvikling: Administrasjon (Admin)",       "milestone": "MP3 – Administration",        "period": "Q4 2026", "priority": "High"},
    "F4": {"title": "Phase 4 – Utvikling: Ferdigstillelse (Completion)", "milestone": "MP4 – Complete Functionality", "period": "Q1 2027", "priority": "High"},
    "F5": {"title": "Phase 5 – Driftssettelse og Stabilisering",         "milestone": "MP5 – Production Readiness",  "period": "Q2 2027", "priority": "High"},
}

# ── Output column layout (A–L, 12 columns) ───────────────────────────────────
COL_WBS        = 1   # A: hierarchical WBS number (1.1.2)
COL_ID         = 2   # B: ID (phase code / EP-* / US-*)
COL_TITLE      = 3   # C: Title
COL_MILESTONE  = 4   # D: Milestone
COL_PERIOD     = 5   # E: Period
COL_PRIORITY   = 6   # F: Priority
COL_STATUS     = 7   # G: Status
COL_POINTS     = 8   # H: Story Points (SUM formula for epic/phase)
COL_HOURS      = 9   # I: Est Hours
COL_START_DATE = 10  # J: Start Date (actual or estimated)
COL_END_DATE   = 11  # K: End Date   (actual or estimated)
COL_NOTES      = 12  # L: Notes
TOTAL_COLS     = 12

WBS_COLUMN_WIDTHS = {
    "A": 10,   # WBS No
    "B": 14,   # ID
    "C": 55,   # Title
    "D": 28,   # Milestone
    "E": 12,   # Period
    "F": 12,   # Priority
    "G": 15,   # Status
    "H": 11,   # Story Pts
    "I": 11,   # Est Hours
    "J": 14,   # Start Date
    "K": 14,   # End Date
    "L": 35,   # Notes
}

DATE_FMT = "YYYY-MM-DD"


# ── Style helpers ─────────────────────────────────────────────────────────────

def _fill(hex_colour: str) -> PatternFill:
    if hex_colour == "00000000":
        return PatternFill(fill_type=None)
    return PatternFill(fill_type="solid", fgColor=hex_colour)


def _font(bold: bool = False, colour: str = COLOUR_WHITE_FG, size: int = 10) -> Font:
    return Font(bold=bold, color=colour, size=size)


def _is_dark(hex_colour: str) -> bool:
    """Return True if the colour is dark enough to warrant white text."""
    rgb = hex_colour[-6:]
    try:
        r, g, b = int(rgb[0:2], 16), int(rgb[2:4], 16), int(rgb[4:6], 16)
    except ValueError:
        return True
    return (0.299 * r + 0.587 * g + 0.114 * b) < 160


def apply_row_style(ws, row_num: int, level: int, col_count: int = TOTAL_COLS):
    if level == 0:
        bg, fg, bold = COLOUR_TITLE_BG,  COLOUR_WHITE_FG, True
    elif level == 1:
        bg, fg, bold = COLOUR_HEADER_BG, COLOUR_WHITE_FG, True
    elif level == 2:
        bg, fg, bold = COLOUR_PHASE_BG,  COLOUR_WHITE_FG, True
    elif level == 3:
        bg, fg, bold = COLOUR_EPIC_BG,   COLOUR_WHITE_FG, True
    elif level == 4:
        bg, fg, bold = COLOUR_STORY_BG,  COLOUR_DARK_FG,  False
    else:
        bg, fg, bold = "00000000",        COLOUR_DARK_FG,  False

    for col in range(1, col_count + 1):
        cell           = ws.cell(row=row_num, column=col)
        cell.fill      = _fill(bg)
        cell.font      = _font(bold=bold, colour=fg)
        cell.alignment = Alignment(vertical="center", wrap_text=False)


def _set_date_cell(cell, d):
    if d is None:
        return
    cell.value         = d
    cell.number_format = DATE_FMT


# ── Date helpers ──────────────────────────────────────────────────────────────

def _parse_iso_date(ts_str: str | None) -> date | None:
    if not ts_str:
        return None
    try:
        return datetime.fromisoformat(ts_str).date()
    except (ValueError, TypeError):
        try:
            return date.fromisoformat(ts_str[:10])
        except (ValueError, TypeError):
            return None


def _add_work_days(start: date, work_days: float) -> date:
    """Advance `start` by `work_days` Mon–Fri days (fractional rounds up)."""
    remaining = float(work_days)
    result    = start
    while remaining > 0:
        result += timedelta(days=1)
        if result.weekday() < 5:
            remaining -= 1.0
    return result


def _work_days_between_inclusive(start: date, end: date) -> int:
    days   = 0
    cursor = start
    while cursor <= end:
        if cursor.weekday() < 5:
            days += 1
        cursor += timedelta(days=1)
    return days


# ── Estimation ────────────────────────────────────────────────────────────────

def _forecast_throughput(forecast: dict | None, velocity: dict, sprint_duration_weeks: int) -> tuple[float, str]:
    throughput = (forecast or {}).get("throughput", {})
    avg_daily  = throughput.get("average", 0) or 0
    observed   = throughput.get("observed_day_count", 0) or 0
    if avg_daily > 0:
        return avg_daily, f"daily throughput over {observed} observed workdays"

    avg_sprint = velocity.get("avg_points_per_sprint", 0) or 0
    work_days  = max(1, sprint_duration_weeks * 5)
    if avg_sprint > 0:
        return avg_sprint / work_days, "sprint velocity fallback"

    return 0.0, "no throughput data"


def _compute_estimates(stories: list, velocity: dict, forecast: dict | None, sprint_duration_weeks: int) -> tuple[dict, float]:
    """
    Return (estimates, hours_per_point).
    estimates: {story_id: {est_hours, est_start, est_end}}
    hours_per_point is 0 if throughput is unknown.
    """
    avg_pts_per_workday, _source = _forecast_throughput(forecast, velocity, sprint_duration_weeks)
    if avg_pts_per_workday <= 0:
        empty = {s["id"]: {"est_hours": None, "est_start": None, "est_end": None} for s in stories}
        return empty, 0.0

    hours_per_day    = 7
    hours_per_point  = hours_per_day / avg_pts_per_workday
    days_per_point   = 1 / avg_pts_per_workday

    STATUS_ORDER = {
        "in-progress": 0, "ready-for-qa": 1, "ready": 2,
        "todo": 3, "draft": 4, "blocked": 5,
    }
    not_done = [s for s in stories if s["status"].lower() not in ("done", "dropped")]
    not_done.sort(key=lambda s: (
        STATUS_ORDER.get(s["status"].lower(), 9),
        s["phase"],
        s.get("epic_id") or "",
        s["id"],
    ))

    estimates: dict     = {}
    today               = date.today()
    cumulative_days     = 0.0

    for s in not_done:
        pts = s.get("story_points") or 0
        if not pts:
            estimates[s["id"]] = {"est_hours": None, "est_start": None, "est_end": None}
            continue

        est_hours    = round(pts * hours_per_point, 1)
        est_duration = pts * days_per_point
        work_started = _parse_iso_date(s.get("work_started"))

        if work_started:
            est_start = work_started
            est_end   = _add_work_days(today, est_duration)
        else:
            est_start        = _add_work_days(today, cumulative_days)
            est_end          = _add_work_days(today, cumulative_days + est_duration)
            cumulative_days += est_duration

        estimates[s["id"]] = {"est_hours": est_hours, "est_start": est_start, "est_end": est_end}

    for s in stories:
        if s["id"] not in estimates:
            estimates[s["id"]] = {
                "est_hours":  None,
                "est_start":  _parse_iso_date(s.get("work_started")),
                "est_end":    _parse_iso_date(s.get("work_done")),
            }

    return estimates, hours_per_point


def _group_dates(stories_in_group: list, estimates: dict) -> tuple[date | None, date | None]:
    """Return (min_start, max_end) across a group of stories."""
    starts, ends = [], []
    for s in stories_in_group:
        sid    = s["id"]
        status = s["status"].lower()
        ws_d   = _parse_iso_date(s.get("work_started"))
        wd_d   = _parse_iso_date(s.get("work_done"))
        est    = estimates.get(sid, {})

        if status == "done":
            if ws_d: starts.append(ws_d)
            if wd_d: ends.append(wd_d)
        elif status in ("in-progress", "ready-for-qa"):
            if ws_d: starts.append(ws_d)
            ee = est.get("est_end")
            if ee:   ends.append(ee)
        else:
            es = est.get("est_start")
            ee = est.get("est_end")
            if es: starts.append(es)
            if ee: ends.append(ee)

    return (min(starts) if starts else None), (max(ends) if ends else None)


# ── Hierarchy builder ─────────────────────────────────────────────────────────

def _build_hierarchy(stories: list) -> list:
    """Return [{phase_id, epics: [{id, title, stories: [...]}]}], all sorted."""
    phase_map: dict = {}
    for s in stories:
        ph      = s["phase"]
        epic_id = s.get("epic_id") or f"(no epic in {ph})"
        epic_t  = s.get("epic_title") or epic_id
        phase_map.setdefault(ph, {}).setdefault(epic_id, {"title": epic_t, "stories": []})
        phase_map[ph][epic_id]["stories"].append(s)

    result = []
    for ph_id in sorted(phase_map):
        epics = []
        for ep_id in sorted(phase_map[ph_id]):
            ed = phase_map[ph_id][ep_id]
            epics.append({
                "id":      ep_id,
                "title":   ed["title"],
                "stories": sorted(ed["stories"], key=lambda s: s["id"]),
            })
        result.append({"id": ph_id, "epics": epics})
    return result


# ── WBS sheet ─────────────────────────────────────────────────────────────────

def _write_title_row(ws, row_num: int, title: str, span: int = TOTAL_COLS):
    ws.row_dimensions[row_num].height = 28
    ws.merge_cells(start_row=row_num, start_column=1, end_row=row_num, end_column=span)
    c            = ws.cell(row=row_num, column=1, value=title)
    c.font       = Font(bold=True, color=COLOUR_WHITE_FG, size=13)
    c.fill       = _fill(COLOUR_TITLE_BG)
    c.alignment  = Alignment(horizontal="left", vertical="center")


def _write_header_row(ws, row_num: int, headers: list):
    ws.row_dimensions[row_num].height = 20
    for col, h in enumerate(headers, start=1):
        c            = ws.cell(row=row_num, column=col, value=h)
        c.font       = _font(bold=True)
        c.fill       = _fill(COLOUR_HEADER_BG)
        c.alignment  = Alignment(horizontal="center", vertical="center")


def _write_phase_row(ws, row_num: int, wbs: str, phase_id: str,
                     ph_start: date | None, ph_end: date | None):
    meta                          = PHASE_META.get(phase_id, {})
    ws.row_dimensions[row_num].height = 20
    ws.cell(row_num, COL_WBS,       value=wbs)
    ws.cell(row_num, COL_ID,        value=phase_id)
    ws.cell(row_num, COL_TITLE,     value=f"   {meta.get('title', phase_id)}")
    ws.cell(row_num, COL_MILESTONE, value=meta.get("milestone", ""))
    ws.cell(row_num, COL_PERIOD,    value=meta.get("period", ""))
    ws.cell(row_num, COL_PRIORITY,  value=meta.get("priority", ""))
    ws.cell(row_num, COL_STATUS,    value="")
    ws.cell(row_num, COL_HOURS,     value=None)
    if ph_start: _set_date_cell(ws.cell(row_num, COL_START_DATE), ph_start)
    if ph_end:   _set_date_cell(ws.cell(row_num, COL_END_DATE),   ph_end)
    apply_row_style(ws, row_num, level=2)


def _write_epic_row(ws, row_num: int, wbs: str, epic_id: str, epic_title: str,
                    phase_id: str, ep_start: date | None, ep_end: date | None):
    meta                          = PHASE_META.get(phase_id, {})
    ws.row_dimensions[row_num].height = 18
    ws.cell(row_num, COL_WBS,       value=wbs)
    ws.cell(row_num, COL_ID,        value=epic_id)
    ws.cell(row_num, COL_TITLE,     value=f"   {epic_title}")
    ws.cell(row_num, COL_MILESTONE, value=meta.get("milestone", ""))
    ws.cell(row_num, COL_PERIOD,    value=meta.get("period", ""))
    ws.cell(row_num, COL_PRIORITY,  value=meta.get("priority", ""))
    ws.cell(row_num, COL_STATUS,    value="")
    ws.cell(row_num, COL_HOURS,     value=None)
    if ep_start: _set_date_cell(ws.cell(row_num, COL_START_DATE), ep_start)
    if ep_end:   _set_date_cell(ws.cell(row_num, COL_END_DATE),   ep_end)
    apply_row_style(ws, row_num, level=3)


def _write_story_row(ws, row_num: int, wbs: str, story: dict,
                     estimates: dict, hours_per_point: float):
    sid                           = story["id"]
    status                        = story["status"].lower()
    est                           = estimates.get(sid, {})
    ws.row_dimensions[row_num].height = 17

    ws.cell(row_num, COL_WBS,    value=wbs)
    ws.cell(row_num, COL_ID,     value=sid)
    ws.cell(row_num, COL_TITLE,  value=f"      {story['title']}")
    ws.cell(row_num, COL_STATUS, value=STATUS_MAP.get(status, status.upper()))
    ws.cell(row_num, COL_POINTS, value=story.get("story_points"))

    pts = story.get("story_points") or 0
    if status in ("done", "in-progress", "ready-for-qa"):
        if pts and hours_per_point > 0:
            ws.cell(row_num, COL_HOURS, value=round(pts * hours_per_point, 1))
    elif est.get("est_hours") is not None:
        ws.cell(row_num, COL_HOURS, value=est["est_hours"])

    ws_date = _parse_iso_date(story.get("work_started"))
    wd_date = _parse_iso_date(story.get("work_done"))

    if status == "done":
        if ws_date: _set_date_cell(ws.cell(row_num, COL_START_DATE), ws_date)
        if wd_date: _set_date_cell(ws.cell(row_num, COL_END_DATE),   wd_date)
    elif status in ("in-progress", "ready-for-qa"):
        if ws_date: _set_date_cell(ws.cell(row_num, COL_START_DATE), ws_date)
        ee = est.get("est_end")
        if ee:      _set_date_cell(ws.cell(row_num, COL_END_DATE),   ee)
    else:
        es = est.get("est_start")
        ee = est.get("est_end")
        if es: _set_date_cell(ws.cell(row_num, COL_START_DATE), es)
        if ee: _set_date_cell(ws.cell(row_num, COL_END_DATE),   ee)

    apply_row_style(ws, row_num, level=4)

    if status == "in-progress":
        status_fill = _fill(COLOUR_STORY_INPROGRESS_BG)
        for col in range(1, TOTAL_COLS + 1):
            ws.cell(row=row_num, column=col).fill = status_fill
    elif status == "done":
        status_fill = _fill(COLOUR_STORY_DONE_BG)
        for col in range(1, TOTAL_COLS + 1):
            ws.cell(row=row_num, column=col).fill = status_fill


def build_wbs_sheet(ws, hierarchy: list, estimates: dict,
                    hours_per_point: float, generated_at: str):
    for col_letter, width in WBS_COLUMN_WIDTHS.items():
        ws.column_dimensions[col_letter].width = width

    report_date = date.fromisoformat(generated_at[:10])
    _write_title_row(ws, 1, f"AutoPASS IP 2.0 – WBS – Report {report_date.strftime('%Y-%m-%d')}")
    _write_header_row(ws, 2, [
        "WBS No", "ID", "Title", "Milestone", "Period", "Priority",
        "Status", "Story Pts", "Est Hours", "Start Date", "End Date", "Notes",
    ])

    row    = 3
    ph_num = 0

    for phase in hierarchy:
        ph_id  = phase["id"]
        ph_num += 1
        ph_wbs  = str(ph_num)

        phase_row            = row
        row                 += 1
        epic_rows_this_phase = []

        ep_num = 0
        for epic in phase["epics"]:
            ep_num  += 1
            ep_wbs   = f"{ph_wbs}.{ep_num}"
            epic_row = row
            epic_rows_this_phase.append(epic_row)
            row     += 1

            first_story_row = row
            st_num          = 0

            for story in epic["stories"]:
                st_num += 1
                _write_story_row(ws, row, f"{ep_wbs}.{st_num}", story, estimates, hours_per_point)
                row    += 1

            last_story_row = row - 1
            ep_start, ep_end = _group_dates(epic["stories"], estimates)
            _write_epic_row(ws, epic_row, ep_wbs, epic["id"], epic["title"],
                            ph_id, ep_start, ep_end)

            pts_formula = (f"=SUM(H{first_story_row}:H{last_story_row})"
                           if last_story_row >= first_story_row else 0)
            ws.cell(epic_row, COL_POINTS).value = pts_formula
            apply_row_style(ws, epic_row, level=3)

        all_phase_stories = [s for ep in phase["epics"] for s in ep["stories"]]
        ph_start, ph_end  = _group_dates(all_phase_stories, estimates)
        _write_phase_row(ws, phase_row, ph_wbs, ph_id, ph_start, ph_end)

        if epic_rows_this_phase:
            refs = ",".join(f"H{r}" for r in epic_rows_this_phase)
            ws.cell(phase_row, COL_POINTS).value = f"=SUM({refs})"
        else:
            ws.cell(phase_row, COL_POINTS).value = 0
        apply_row_style(ws, phase_row, level=2)

    ws.sheet_view.showGridLines = False


# ── Phase Summary sheet ───────────────────────────────────────────────────────

def build_phase_summary_sheet(ws, phases: list, stories: list):
    epics_by_phase: dict = {}
    for s in stories:
        epics_by_phase.setdefault(s["phase"], set()).add(s.get("epic_id") or "?")

    for col, width in zip("ABCDEFGHIJ", [10, 55, 12, 30, 8, 9, 12, 12, 13, 13]):
        ws.column_dimensions[col].width = width

    _write_title_row(ws, 1, "AutoPASS IP 2.0 — Phase & Milestone Summary", span=10)
    _write_header_row(ws, 2, [
        "Phase", "Title", "Period", "Milestone", "Epics", "Stories",
        "Pts Total", "Pts Done", "Pts In Progress", "Pts Remaining",
    ])

    totals = {"epics": 0, "stories": 0, "total": 0, "done": 0, "wip": 0, "remaining": 0}
    row    = 3
    for ph_dto in sorted(phases, key=lambda p: p["phase"]):
        ph_id      = ph_dto["phase"]
        meta       = PHASE_META.get(ph_id, {})
        epic_count = len(epics_by_phase.get(ph_id, set()))
        ws.row_dimensions[row].height = 18

        data = [
            ph_id,
            meta.get("title", ph_id),
            meta.get("period", ""),
            meta.get("milestone", ""),
            epic_count,
            ph_dto["story_count"],
            ph_dto["points_total"],
            ph_dto["points_done"],
            ph_dto["points_in_progress"],
            ph_dto["points_remaining"],
        ]
        for col, val in enumerate(data, start=1):
            c            = ws.cell(row=row, column=col, value=val)
            c.font       = _font(bold=False, colour=COLOUR_DARK_FG)
            c.alignment  = Alignment(horizontal="left" if col <= 4 else "center", vertical="center")
            if col >= 5:
                c.fill   = _fill("FFE8F0FA")

        totals["epics"]     += epic_count
        totals["stories"]   += ph_dto["story_count"]
        totals["total"]     += ph_dto["points_total"]
        totals["done"]      += ph_dto["points_done"]
        totals["wip"]       += ph_dto["points_in_progress"]
        totals["remaining"] += ph_dto["points_remaining"]
        row += 1

    ws.row_dimensions[row].height = 20
    for col, val in enumerate(
        ["TOTAL", "", "", "", totals["epics"], totals["stories"],
         totals["total"], totals["done"], totals["wip"], totals["remaining"]],
        start=1,
    ):
        c            = ws.cell(row=row, column=col, value=val)
        c.font       = Font(bold=True, color=COLOUR_WHITE_FG, size=10)
        c.fill       = _fill(COLOUR_PHASE_BG)
        c.alignment  = Alignment(horizontal="left" if col <= 4 else "center", vertical="center")

    ws.sheet_view.showGridLines = False


# ── Sprint Burndown sheet ─────────────────────────────────────────────────────

def build_sprint_burndown_sheet(ws, sprints: list, velocity: dict, forecast: dict, generated_at: str):
    for col, width in zip("ABCDEFGH", [28, 13, 13, 14, 14, 14, 16, 18]):
        ws.column_dimensions[col].width = width

    _write_title_row(ws, 1, "AutoPASS IP 2.0 — Sprint Burndown & Prognosis", span=8)

    avg       = velocity.get("avg_points_per_sprint", 0)
    remaining = velocity.get("remaining_points", 0)
    completed = velocity.get("completed_sprint_count", 0)
    dur       = velocity.get("sprint_duration_weeks", 2)
    completion = forecast.get("completion", {}) if forecast else {}
    throughput = forecast.get("throughput", {}) if forecast else {}
    daily_avg, forecast_source = _forecast_throughput(forecast, velocity, dur)
    observed_days = throughput.get("observed_day_count", 0) or 0
    p50 = completion.get("p50_date")
    p80 = completion.get("p80_date")
    p90 = completion.get("p90_date")
    est_text = f"P50 {p50} / P80 {p80} / P90 {p90}" if p80 else "-"

    ws.merge_cells("A2:H2")
    ws.row_dimensions[2].height = 16
    sc            = ws["A2"]
    sc.value      = (
        f"Throughput: {daily_avg:.1f} pts/workday (over {observed_days} observed workdays)  ·  "
        f"Sprint velocity: {avg:.1f} pts/sprint (over {completed} completed sprint{'s' if completed != 1 else ''})  ·  "
        f"Remaining: {remaining} pts  ·  Forecast completion: {est_text}  ·  Generated: {generated_at[:10]}"
    )
    sc.font       = Font(italic=True, color=COLOUR_WHITE_FG, size=9)
    sc.fill       = _fill(COLOUR_HEADER_BG)
    sc.alignment  = Alignment(horizontal="left", vertical="center")

    _write_header_row(ws, 3, [
        "Sprint", "Start", "End", "Planned Pts", "Delivered Pts",
        "Rate (avg)", "Remaining (cum.)", "Status",
    ])

    total_delivered_all  = sum(s.get("delivered_points", 0) for s in sprints)
    cumulative_remaining = velocity.get("remaining_points", 0) + total_delivered_all
    row                  = 4

    for s in sprints:
        is_past    = s.get("is_past", False)
        is_current = s.get("is_current", False)
        if is_past:
            status_str, row_bg = "completed", "FFEBF5EB"
        elif is_current:
            status_str, row_bg = "active",    "FFFFF3CD"
        else:
            status_str, row_bg = "planned",   "FFFFFFFF"

        ws.row_dimensions[row].height = 17
        cumulative_remaining -= s.get("delivered_points", 0)
        row_data = [
            s["sprint_name"],
            s["start_date"],
            s["end_date"],
            s.get("planned_points", 0) or None,
            s.get("delivered_points", 0) if (is_past or is_current) else None,
            avg if is_past and s.get("planned_points", 0) > 0 else None,
            cumulative_remaining if (is_past or is_current) else None,
            status_str,
        ]
        for col, val in enumerate(row_data, start=1):
            c            = ws.cell(row=row, column=col, value=val)
            c.font       = Font(color=COLOUR_DARK_FG, size=10)
            c.fill       = PatternFill(fill_type="solid", fgColor=row_bg[2:])
            c.alignment  = Alignment(horizontal="left" if col == 1 else "center", vertical="center")
            if col in (4, 5, 6, 7) and val is not None:
                c.number_format = "0"
        row += 1

    if p80 and daily_avg > 0 and sprints:
        last_end       = datetime.strptime(sprints[-1]["end_date"], "%Y-%m-%d").date()
        sprint_days    = dur * 7
        proj_remaining = cumulative_remaining
        sprint_num     = 1

        ws.row_dimensions[row].height = 4
        row += 1

        ws.merge_cells(f"A{row}:H{row}")
        ws.row_dimensions[row].height = 17
        c            = ws.cell(row=row, column=1, value=f"▸ Projected future sprints (based on {forecast_source})")
        c.font       = Font(bold=True, italic=True, color="FF444444", size=9)
        c.alignment  = Alignment(horizontal="left", vertical="center")
        row         += 1

        while proj_remaining > 0 and sprint_num <= 40:
            proj_start         = last_end + timedelta(days=1 + (sprint_num - 1) * sprint_days)
            proj_end           = proj_start + timedelta(days=sprint_days - 1)
            work_days          = _work_days_between_inclusive(proj_start, proj_end)
            projected_delivery = min(daily_avg * work_days, proj_remaining)
            proj_remaining     = max(0, proj_remaining - projected_delivery)

            ws.row_dimensions[row].height = 16
            row_data = [
                f"S{len(sprints) + sprint_num:03d}.projected",
                proj_start.strftime("%Y-%m-%d"),
                proj_end.strftime("%Y-%m-%d"),
                round(daily_avg * work_days),
                round(projected_delivery),
                daily_avg,
                max(0, proj_remaining),
                "projected",
            ]
            for col, val in enumerate(row_data, start=1):
                c            = ws.cell(row=row, column=col, value=val)
                c.font       = Font(color="FF888888", italic=True, size=9)
                c.fill       = PatternFill(fill_type="solid", fgColor="F5F5F5")
                c.alignment  = Alignment(horizontal="left" if col == 1 else "center", vertical="center")
                if col in (4, 5, 6, 7):
                    c.number_format = "0"
            row       += 1
            sprint_num += 1

    ws.sheet_view.showGridLines = False


# ── Legend sheet ──────────────────────────────────────────────────────────────

def build_legend_sheet(ws):
    for col, width in zip("ABC", [22, 50, 20]):
        ws.column_dimensions[col].width = width

    _write_title_row(ws, 1, "AutoPASS IP 2.0 — Legend & Guide", span=3)

    sections = [
        ("Row types", [
            (COLOUR_PHASE_BG,            "Phase",       "Top-level project phase (F1–F5)"),
            (COLOUR_EPIC_BG,             "Epic",        "Epic grouping user stories within a phase"),
            (COLOUR_STORY_BG,            "Story",       "User story (default / not started)"),
        ]),
        ("Story status colours", [
            (COLOUR_STORY_INPROGRESS_BG, "In Progress", "Story currently being developed"),
            (COLOUR_STORY_DONE_BG,       "Done",        "Completed and accepted story"),
        ]),
        ("WBS columns", [
            (None, "WBS No",     "Hierarchical number (phase.epic.story)"),
            (None, "ID",         "Artifact ID — Fn / EP-Fn-* / US-Fn-*"),
            (None, "Title",      "Phase, epic, or story title"),
            (None, "Milestone",  "Delivery milestone (MP1–MP5)"),
            (None, "Period",     "Target delivery quarter"),
            (None, "Priority",   "Critical / High / Medium / Low"),
            (None, "Status",     "Current workflow status"),
            (None, "Story Pts",  "Estimated story points; SUM for epic/phase rows"),
            (None, "Est Hours",  "Estimated hours (throughput-based)"),
            (None, "Start Date", "Actual start (done/in-progress) or estimated"),
            (None, "End Date",   "Actual end (done) or throughput-based estimate"),
            (None, "Notes",      "Free-text remarks"),
        ]),
    ]

    row = 3
    for section_title, entries in sections:
        ws.row_dimensions[row].height = 18
        ws.merge_cells(start_row=row, start_column=1, end_row=row, end_column=3)
        hc            = ws.cell(row=row, column=1, value=section_title)
        hc.font       = Font(bold=True, color=COLOUR_WHITE_FG, size=10)
        hc.fill       = _fill(COLOUR_HEADER_BG)
        hc.alignment  = Alignment(horizontal="left", vertical="center")
        row          += 1

        for colour, label, desc in entries:
            ws.row_dimensions[row].height = 17
            fg            = COLOUR_WHITE_FG if colour and _is_dark(colour) else COLOUR_DARK_FG
            swatch        = ws.cell(row=row, column=1, value=label)
            swatch.font   = Font(bold=bool(colour), color=fg, size=10)
            swatch.fill   = _fill(colour) if colour else PatternFill(fill_type=None)
            swatch.alignment = Alignment(horizontal="left", vertical="center")

            desc_cell           = ws.cell(row=row, column=2, value=desc)
            desc_cell.font      = Font(color=COLOUR_DARK_FG, size=10)
            desc_cell.alignment = Alignment(horizontal="left", vertical="center")
            row                += 1

        row += 1  # blank separator between sections

    ws.sheet_view.showGridLines = False


# ── Output path helpers ───────────────────────────────────────────────────────

REPORT_DIR      = Path("delivery/reports")
REPORT_BASENAME = "autopass_ip_2.0_wbs_report.xlsx"


def _next_output_path() -> Path:
    """Return delivery/reports/<today>.<nnn>.autopass_ip_2.0_wbs_report.xlsx.

    Scans the directory for existing files with today's date prefix and picks
    the next sequence number, starting at 001.
    """
    today = date.today().strftime("%Y-%m-%d")
    REPORT_DIR.mkdir(parents=True, exist_ok=True)
    existing = sorted(REPORT_DIR.glob(f"{today}.???.{REPORT_BASENAME}"))
    seq      = (int(existing[-1].name.split(".")[1]) + 1) if existing else 1
    return REPORT_DIR / f"{today}.{seq:03d}.{REPORT_BASENAME}"


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Generate WBS xlsx report from kanban JSON data (read from stdin)."
    )
    parser.add_argument(
        "--output",
        default=None,
        help=(
            "Output path for the generated report (.xlsx). "
            f"Defaults to {REPORT_DIR}/<date>.<nnn>.{REPORT_BASENAME}"
        ),
    )
    args = parser.parse_args()

    output_path = Path(args.output) if args.output else _next_output_path()

    try:
        envelope = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        print(f"ERROR: Failed to parse JSON from stdin: {e}", file=sys.stderr)
        sys.exit(1)

    if envelope.get("status") != "ok":
        msg = envelope.get("error", {}).get("message", "unknown error")
        print(f"ERROR: kanban reported an error: {msg}", file=sys.stderr)
        sys.exit(1)

    data         = envelope["data"]
    stories      = data["stories"]
    sprints      = data["sprints"]
    phases       = data["phases"]
    velocity     = data["velocity"]
    forecast     = data["forecast"]
    generated_at = data["generated_at"]

    hierarchy              = _build_hierarchy(stories)
    sprint_dur             = velocity.get("sprint_duration_weeks", 2)
    estimates, hpp         = _compute_estimates(stories, velocity, forecast, sprint_dur)
    avg_daily, source      = _forecast_throughput(forecast, velocity, sprint_dur)

    wb = openpyxl.Workbook()

    ws_wbs       = wb.active
    ws_wbs.title = "WBS – AutoPASS IP 2.0"
    print("Building WBS sheet …", file=sys.stderr)
    build_wbs_sheet(ws_wbs, hierarchy, estimates, hpp, generated_at)

    print("Building Phase Summary sheet …", file=sys.stderr)
    build_phase_summary_sheet(wb.create_sheet("Phase Summary"), phases, stories)

    print("Building Sprint Burndown sheet …", file=sys.stderr)
    build_sprint_burndown_sheet(wb.create_sheet("Sprint Burndown"), sprints, velocity, forecast, generated_at)

    print("Building Legend sheet …", file=sys.stderr)
    build_legend_sheet(wb.create_sheet("Legend & Guide"))

    output_path.parent.mkdir(parents=True, exist_ok=True)
    wb.save(str(output_path))
    print(f"Report saved: {output_path}", file=sys.stderr)
    print(f"  Stories: {len(stories)}", file=sys.stderr)
    print(f"  Sprints: {len(sprints)}", file=sys.stderr)
    print(f"  Phases:  {len(phases)}", file=sys.stderr)
    if hpp > 0:
        print(
            f"  Hours/point: {hpp:.1f}h  "
            f"({source}, {avg_daily:.1f} pts/workday)",
            file=sys.stderr,
        )
    completion = forecast.get("completion", {})
    if completion.get("p80_date"):
        print(
            f"  Forecast: P50 {completion.get('p50_date')} / "
            f"P80 {completion.get('p80_date')} / P90 {completion.get('p90_date')} "
            f"({forecast.get('confidence', 'unknown')} confidence)",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
