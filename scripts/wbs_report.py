#!/usr/bin/env python3
"""
Generate an updated WBS Excel report from AutoPASS IP 2.0 kanban data.

Usage:
    kanban --format json report wbs | python3 tools/kanban/scripts/wbs_report.py \\
        --template delivery/backlog/2026-03-31.autopass_ip_2.0_wbs.xlsx \\
        --output delivery/backlog/wbs_report.xlsx

The script reads JSON from stdin (produced by `kanban --format json report wbs`),
reads the WBS template for style/column-width and per-story field data, then writes
an updated xlsx with:
  - Hierarchical WBS numbering (phase.epic.story) rebuilt from live data
  - New/renumbered stories inserted in correct phase/epic position
  - SUM formulas for story-point totals on epic and phase rows
  - Start Date (J) and End Date (K) columns: actual for done/in-progress stories,
    velocity-based estimates for not-yet-started stories
  - Estimated hours (I) derived from velocity for unstarted stories
  - Sprint burndown prognosis sheet
  - Report date appended to the A1 heading
"""

import argparse
import json
import re
import sys
from datetime import date, datetime, timedelta
from pathlib import Path
import copy as _copy

try:
    import openpyxl
    from openpyxl.styles import Alignment, Border, Font, PatternFill, Side
    from openpyxl.utils import get_column_letter
except ImportError:
    print("ERROR: openpyxl is required. Install with: pip3 install openpyxl", file=sys.stderr)
    sys.exit(1)


# ── Colour palette ────────────────────────────────────────────────────────────

COLOUR_TITLE_BG  = "FF0D1F40"
COLOUR_HEADER_BG = "FF1A3060"
COLOUR_PHASE_BG  = "FF1F3864"
COLOUR_EPIC_BG   = "FF2E5EAA"
COLOUR_STORY_BG  = "FFD6E4F0"
COLOUR_WHITE_FG  = "FFFFFFFF"
COLOUR_DARK_FG   = "FF1F1F1F"

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
    "F1": {"title": "Phase 1 – Etablering (Establishment)",              "milestone": "MP1 – Foundation",           "period": "Q2 2026", "priority": "Critical"},
    "F2": {"title": "Phase 2 – Utvikling: Kjernelogikk (Core Logic)",    "milestone": "MP2 – Core Logic",           "period": "Q3 2026", "priority": "Critical"},
    "F3": {"title": "Phase 3 – Utvikling: Administrasjon (Admin)",       "milestone": "MP3 – Administration",       "period": "Q4 2026", "priority": "High"},
    "F4": {"title": "Phase 4 – Utvikling: Ferdigstillelse (Completion)", "milestone": "MP4 – Complete Functionality","period": "Q1 2027", "priority": "High"},
    "F5": {"title": "Phase 5 – Driftssettelse og Stabilisering",         "milestone": "MP5 – Production Readiness", "period": "Q2 2027", "priority": "High"},
}

# ── Output column layout (A–L, 12 columns) ────────────────────────────────────
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

DATE_FMT = "YYYY-MM-DD"


# ── Style helpers ─────────────────────────────────────────────────────────────

def _fill(hex_colour: str) -> PatternFill:
    if hex_colour == "00000000":
        return PatternFill(fill_type=None)
    return PatternFill(fill_type="solid", fgColor=hex_colour)


def _font(bold: bool = False, colour: str = COLOUR_WHITE_FG, size: int = 10) -> Font:
    return Font(bold=bold, color=colour, size=size)


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
        cell = ws.cell(row=row_num, column=col)
        cell.fill  = _fill(bg)
        cell.font  = _font(bold=bold, colour=fg)
        cell.alignment = Alignment(vertical="center", wrap_text=False)


def _set_date_cell(cell, d):
    """Write a Python date object to cell with ISO 8601 display format."""
    if d is None:
        return
    cell.value = d
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
    result = start
    while remaining > 0:
        result += timedelta(days=1)
        if result.weekday() < 5:  # Mon–Fri
            remaining -= 1.0
    return result


# ── Estimation ────────────────────────────────────────────────────────────────

def _compute_estimates(stories: list, velocity: dict, sprint_duration_weeks: int) -> tuple[dict, float]:
    """
    Return (estimates, hours_per_point).
    estimates: {story_id: {'est_hours': float|None, 'est_start': date|None, 'est_end': date|None}}
    hours_per_point is 0 if velocity is unknown.
    """
    avg_pts = velocity.get("avg_points_per_sprint", 0) or 0
    if avg_pts <= 0:
        empty = {s["id"]: {"est_hours": None, "est_start": None, "est_end": None} for s in stories}
        return empty, 0.0

    hours_per_day    = 7
    work_days_sprint = sprint_duration_weeks * 5
    hours_per_sprint = work_days_sprint * hours_per_day
    hours_per_point  = hours_per_sprint / avg_pts
    days_per_point   = work_days_sprint / avg_pts

    # Sort: in-progress first (already started), then by phase/epic/story
    STATUS_ORDER = {
        "in-progress": 0, "ready-for-qa": 1, "ready": 2,
        "todo": 3, "draft": 4, "blocked": 5,
    }
    not_done = [
        s for s in stories
        if s["status"].lower() not in ("done", "dropped")
    ]
    not_done.sort(key=lambda s: (
        STATUS_ORDER.get(s["status"].lower(), 9),
        s["phase"],
        s.get("epic_id") or "",
        s["id"],
    ))

    estimates: dict = {}
    today = date.today()
    cumulative_days = 0.0

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
            est_start = _add_work_days(today, cumulative_days)
            est_end   = _add_work_days(today, cumulative_days + est_duration)
            cumulative_days += est_duration

        estimates[s["id"]] = {"est_hours": est_hours, "est_start": est_start, "est_end": est_end}

    for s in stories:
        if s["id"] not in estimates:
            ws_date = _parse_iso_date(s.get("work_started"))
            wd_date = _parse_iso_date(s.get("work_done"))
            estimates[s["id"]] = {"est_hours": None, "est_start": ws_date, "est_end": wd_date}

    return estimates, hours_per_point


def _group_dates(stories_in_group: list, estimates: dict) -> tuple[date | None, date | None]:
    """Return (min_start, max_end) across a group of stories."""
    starts, ends = [], []
    for s in stories_in_group:
        sid    = s["id"]
        status = s["status"].lower()
        ws     = _parse_iso_date(s.get("work_started"))
        wd     = _parse_iso_date(s.get("work_done"))
        est    = estimates.get(sid, {})

        if status == "done":
            if ws: starts.append(ws)
            if wd: ends.append(wd)
        elif status in ("in-progress", "ready-for-qa"):
            if ws: starts.append(ws)
            ee = est.get("est_end")
            if ee: ends.append(ee)
        else:
            es = est.get("est_start")
            ee = est.get("est_end")
            if es: starts.append(es)
            if ee: ends.append(ee)

    return (min(starts) if starts else None), (max(ends) if ends else None)


# ── Hierarchy builder ─────────────────────────────────────────────────────────

def _build_hierarchy(stories: list) -> list:
    """
    Return [{phase_id, epics: [{id, title, stories: [...]}]}], all sorted.
    """
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


# ── Template data extraction ──────────────────────────────────────────────────

def _extract_template_data(wb_template) -> tuple[dict, dict]:
    """
    Extract column widths and per-ID lookup from the template WBS sheet.
    Returns (col_widths, row_data) where row_data maps ID → {milestone, period, priority, notes}.
    Notes are read from whichever column header says 'notes' (old J or new L).
    """
    ws = wb_template["WBS – AutoPASS IP 2.0"]

    col_widths = {k: v.width for k, v in ws.column_dimensions.items() if v.width}

    # Discover the Notes column by scanning header row
    notes_col = 10  # default: old template column J
    for row in ws.iter_rows(min_row=1, max_row=3):
        for cell in row:
            if cell.value and isinstance(cell.value, str) and "notes" in cell.value.lower():
                notes_col = cell.column
                break

    row_data: dict = {}
    for row in ws.iter_rows(min_row=2):
        if len(row) < COL_ID:
            continue
        raw_id = row[COL_ID - 1].value
        if not raw_id:
            continue
        rid = str(raw_id).strip()
        milestone = row[COL_MILESTONE - 1].value if len(row) >= COL_MILESTONE else None
        period    = row[COL_PERIOD - 1].value    if len(row) >= COL_PERIOD    else None
        priority  = row[COL_PRIORITY - 1].value  if len(row) >= COL_PRIORITY  else None
        notes     = row[notes_col - 1].value     if len(row) >= notes_col     else None
        row_data[rid] = {
            "milestone": milestone,
            "period":    period,
            "priority":  priority,
            "notes":     notes,
        }

    return col_widths, row_data


# ── WBS sheet builder ─────────────────────────────────────────────────────────

def _write_title_row(ws, row_num: int, title: str):
    ws.row_dimensions[row_num].height = 28
    ws.merge_cells(start_row=row_num, start_column=1, end_row=row_num, end_column=TOTAL_COLS)
    c = ws.cell(row=row_num, column=1, value=title)
    c.font      = Font(bold=True, color=COLOUR_WHITE_FG, size=13)
    c.fill      = _fill(COLOUR_TITLE_BG)
    c.alignment = Alignment(horizontal="left", vertical="center")


def _write_header_row(ws, row_num: int):
    ws.row_dimensions[row_num].height = 20
    headers = [
        "WBS No", "ID", "Title", "Milestone", "Period", "Priority",
        "Status", "Story Pts", "Est Hours", "Start Date", "End Date", "Notes",
    ]
    for col, h in enumerate(headers, start=1):
        c = ws.cell(row=row_num, column=col, value=h)
        c.font      = _font(bold=True)
        c.fill      = _fill(COLOUR_HEADER_BG)
        c.alignment = Alignment(horizontal="center", vertical="center")


def _write_phase_row(ws, row_num: int, wbs: str, phase_id: str,
                     tdata: dict, ph_start: date | None, ph_end: date | None):
    meta = PHASE_META.get(phase_id, {})
    td   = tdata.get(phase_id, {})
    ws.row_dimensions[row_num].height = 20

    ws.cell(row_num, COL_WBS,       value=wbs)
    ws.cell(row_num, COL_ID,        value=phase_id)
    ws.cell(row_num, COL_TITLE,     value=f"   {meta.get('title', phase_id)}")
    ws.cell(row_num, COL_MILESTONE, value=td.get("milestone") or meta.get("milestone", ""))
    ws.cell(row_num, COL_PERIOD,    value=td.get("period")    or meta.get("period", ""))
    ws.cell(row_num, COL_PRIORITY,  value=td.get("priority")  or meta.get("priority", ""))
    ws.cell(row_num, COL_STATUS,    value="")
    # COL_POINTS set later (SUM formula)
    ws.cell(row_num, COL_HOURS,     value=None)
    if ph_start: _set_date_cell(ws.cell(row_num, COL_START_DATE), ph_start)
    if ph_end:   _set_date_cell(ws.cell(row_num, COL_END_DATE),   ph_end)
    ws.cell(row_num, COL_NOTES,     value=td.get("notes"))
    apply_row_style(ws, row_num, level=2)


def _write_epic_row(ws, row_num: int, wbs: str, epic_id: str, epic_title: str,
                    tdata: dict, phase_id: str,
                    ep_start: date | None, ep_end: date | None):
    meta = PHASE_META.get(phase_id, {})
    td   = tdata.get(epic_id, {})
    ws.row_dimensions[row_num].height = 18

    ws.cell(row_num, COL_WBS,       value=wbs)
    ws.cell(row_num, COL_ID,        value=epic_id)
    ws.cell(row_num, COL_TITLE,     value=f"   {epic_title}")
    ws.cell(row_num, COL_MILESTONE, value=td.get("milestone") or meta.get("milestone", ""))
    ws.cell(row_num, COL_PERIOD,    value=td.get("period")    or meta.get("period", ""))
    ws.cell(row_num, COL_PRIORITY,  value=td.get("priority")  or meta.get("priority", ""))
    ws.cell(row_num, COL_STATUS,    value="")
    # COL_POINTS set later
    ws.cell(row_num, COL_HOURS,     value=None)
    if ep_start: _set_date_cell(ws.cell(row_num, COL_START_DATE), ep_start)
    if ep_end:   _set_date_cell(ws.cell(row_num, COL_END_DATE),   ep_end)
    ws.cell(row_num, COL_NOTES,     value=td.get("notes"))
    apply_row_style(ws, row_num, level=3)


def _write_story_row(ws, row_num: int, wbs: str, story: dict,
                     tdata: dict, estimates: dict, hours_per_point: float):
    sid    = story["id"]
    status = story["status"].lower()
    td     = tdata.get(sid, {})
    est    = estimates.get(sid, {})

    ws.row_dimensions[row_num].height = 17

    ws.cell(row_num, COL_WBS,       value=wbs)
    ws.cell(row_num, COL_ID,        value=sid)
    ws.cell(row_num, COL_TITLE,     value=f"      {story['title']}")
    ws.cell(row_num, COL_MILESTONE, value=td.get("milestone", ""))
    ws.cell(row_num, COL_PERIOD,    value=td.get("period", ""))
    ws.cell(row_num, COL_PRIORITY,  value=td.get("priority", ""))
    ws.cell(row_num, COL_STATUS,    value=STATUS_MAP.get(status, status.upper()))
    ws.cell(row_num, COL_POINTS,    value=story.get("story_points"))

    pts = story.get("story_points") or 0
    if status in ("done", "in-progress", "ready-for-qa"):
        # Actual hours not tracked — show estimate for reference
        if pts and hours_per_point > 0:
            ws.cell(row_num, COL_HOURS, value=round(pts * hours_per_point, 1))
    else:
        if est.get("est_hours") is not None:
            ws.cell(row_num, COL_HOURS, value=est["est_hours"])

    # Dates
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

    ws.cell(row_num, COL_NOTES, value=td.get("notes"))
    apply_row_style(ws, row_num, level=4)


def build_wbs_sheet(ws_src, ws_dst, stories_by_id: dict, hierarchy: list,
                    estimates: dict, hours_per_point: float,
                    tdata: dict, col_widths: dict, generated_at: str):
    # ── Column widths ─────────────────────────────────────────────────────────
    for col_letter, width in col_widths.items():
        ws_dst.column_dimensions[col_letter].width = width
    # New columns J, K — set sensible defaults if not in template
    ws_dst.column_dimensions["J"].width = ws_dst.column_dimensions["J"].width or 14
    ws_dst.column_dimensions["K"].width = ws_dst.column_dimensions["K"].width or 14
    ws_dst.column_dimensions["L"].width = ws_dst.column_dimensions["L"].width or 30

    # ── Row 1: Title ──────────────────────────────────────────────────────────
    raw_title = ws_src.cell(row=1, column=1).value or "AutoPASS IP 2.0 – WBS"
    clean_title = re.sub(r"\s*[-–—]\s*Report\s+.*$", "", str(raw_title), flags=re.IGNORECASE).strip()
    report_date = date.fromisoformat(generated_at[:10])
    title = f"{clean_title} – Report {report_date.strftime('%Y-%m-%d')}"
    _write_title_row(ws_dst, 1, title)

    # ── Row 2: Headers ────────────────────────────────────────────────────────
    _write_header_row(ws_dst, 2)

    # ── Data rows ─────────────────────────────────────────────────────────────
    row = 3
    ph_num = 0

    for phase in hierarchy:
        ph_id  = phase["id"]
        ph_num += 1
        ph_wbs  = str(ph_num)

        phase_row = row
        row += 1
        epic_rows_this_phase = []

        ep_num = 0
        for epic in phase["epics"]:
            ep_num  += 1
            ep_wbs   = f"{ph_wbs}.{ep_num}"
            epic_row = row
            epic_rows_this_phase.append(epic_row)
            row += 1

            first_story_row = row
            st_num = 0

            for story in epic["stories"]:
                st_num += 1
                st_wbs  = f"{ep_wbs}.{st_num}"
                _write_story_row(ws_dst, row, st_wbs, story, tdata, estimates, hours_per_point)
                row += 1

            last_story_row = row - 1

            # Epic dates
            ep_start, ep_end = _group_dates(epic["stories"], estimates)

            # Write epic header (dates known now)
            _write_epic_row(ws_dst, epic_row, ep_wbs, epic["id"], epic["title"],
                            tdata, ph_id, ep_start, ep_end)

            # Epic SUM formula for story points
            if last_story_row >= first_story_row:
                ws_dst.cell(epic_row, COL_POINTS).value = f"=SUM(H{first_story_row}:H{last_story_row})"
            else:
                ws_dst.cell(epic_row, COL_POINTS).value = 0
            # Re-apply style so the formula cell keeps correct colour/font
            apply_row_style(ws_dst, epic_row, level=3)

        # Phase dates: aggregate across all stories in phase
        all_phase_stories = [s for ep in phase["epics"] for s in ep["stories"]]
        ph_start, ph_end = _group_dates(all_phase_stories, estimates)

        _write_phase_row(ws_dst, phase_row, ph_wbs, ph_id, tdata, ph_start, ph_end)

        # Phase SUM: sum of epic H cells
        if epic_rows_this_phase:
            refs = ",".join(f"H{r}" for r in epic_rows_this_phase)
            ws_dst.cell(phase_row, COL_POINTS).value = f"=SUM({refs})"
        else:
            ws_dst.cell(phase_row, COL_POINTS).value = 0
        apply_row_style(ws_dst, phase_row, level=2)

    ws_dst.sheet_view.showGridLines = False


# ── Phase Summary sheet builder ────────────────────────────────────────────────

def build_phase_summary_sheet(ws, phases: list, stories: list):
    epics_by_phase: dict = {}
    for s in stories:
        ph   = s["phase"]
        epic = s.get("epic_id") or "?"
        epics_by_phase.setdefault(ph, set()).add(epic)

    ws.column_dimensions["A"].width = 10
    ws.column_dimensions["B"].width = 55
    ws.column_dimensions["C"].width = 12
    ws.column_dimensions["D"].width = 30
    ws.column_dimensions["E"].width = 8
    ws.column_dimensions["F"].width = 9
    ws.column_dimensions["G"].width = 12
    ws.column_dimensions["H"].width = 12
    ws.column_dimensions["I"].width = 13
    ws.column_dimensions["J"].width = 13

    ws.row_dimensions[1].height = 28
    ws.merge_cells("A1:J1")
    tc = ws["A1"]
    tc.value     = "AutoPASS IP 2.0 — Phase & Milestone Summary"
    tc.font      = Font(bold=True, color=COLOUR_WHITE_FG, size=13)
    tc.fill      = _fill(COLOUR_TITLE_BG)
    tc.alignment = Alignment(horizontal="left", vertical="center")

    ws.row_dimensions[2].height = 20
    headers = ["Phase", "Title", "Period", "Milestone", "Epics", "Stories",
               "Pts Total", "Pts Done", "Pts In Progress", "Pts Remaining"]
    for col, h in enumerate(headers, start=1):
        c = ws.cell(row=2, column=col, value=h)
        c.font      = _font(bold=True)
        c.fill      = _fill(COLOUR_HEADER_BG)
        c.alignment = Alignment(horizontal="center", vertical="center")

    totals = {"epics": 0, "stories": 0, "total": 0, "done": 0, "wip": 0, "remaining": 0}
    row = 3
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
            c = ws.cell(row=row, column=col, value=val)
            c.font      = _font(bold=False, colour=COLOUR_DARK_FG)
            c.alignment = Alignment(horizontal="left" if col <= 4 else "center", vertical="center")
            if col >= 5:
                c.fill = _fill("FFE8F0FA")

        totals["epics"]     += epic_count
        totals["stories"]   += ph_dto["story_count"]
        totals["total"]     += ph_dto["points_total"]
        totals["done"]      += ph_dto["points_done"]
        totals["wip"]       += ph_dto["points_in_progress"]
        totals["remaining"] += ph_dto["points_remaining"]
        row += 1

    ws.row_dimensions[row].height = 20
    total_row = ["TOTAL", "", "", "", totals["epics"], totals["stories"],
                 totals["total"], totals["done"], totals["wip"], totals["remaining"]]
    for col, val in enumerate(total_row, start=1):
        c = ws.cell(row=row, column=col, value=val)
        c.font      = Font(bold=True, color=COLOUR_WHITE_FG, size=10)
        c.fill      = _fill(COLOUR_PHASE_BG)
        c.alignment = Alignment(horizontal="left" if col <= 4 else "center", vertical="center")


# ── Sprint Burndown sheet builder ─────────────────────────────────────────────

def build_sprint_burndown_sheet(ws, sprints: list, velocity: dict, generated_at: str):
    ws.column_dimensions["A"].width = 28
    ws.column_dimensions["B"].width = 13
    ws.column_dimensions["C"].width = 13
    ws.column_dimensions["D"].width = 14
    ws.column_dimensions["E"].width = 14
    ws.column_dimensions["F"].width = 14
    ws.column_dimensions["G"].width = 16
    ws.column_dimensions["H"].width = 18

    ws.merge_cells("A1:H1")
    ws.row_dimensions[1].height = 28
    title = ws["A1"]
    title.value     = "AutoPASS IP 2.0 — Sprint Burndown & Prognosis"
    title.font      = Font(bold=True, color=COLOUR_WHITE_FG, size=13)
    title.fill      = _fill(COLOUR_TITLE_BG)
    title.alignment = Alignment(horizontal="left", vertical="center")

    ws.merge_cells("A2:H2")
    ws.row_dimensions[2].height = 16
    avg       = velocity.get("avg_points_per_sprint", 0)
    remaining = velocity.get("remaining_points", 0)
    est       = velocity.get("estimated_sprints_remaining")
    completed = velocity.get("completed_sprint_count", 0)
    dur       = velocity.get("sprint_duration_weeks", 2)
    est_text  = f"{est:.1f} sprints" if est else "—"
    est_weeks = f" ({est * dur:.0f} weeks)" if est else ""
    sc = ws["A2"]
    sc.value = (
        f"Velocity: {avg:.1f} pts/sprint (over {completed} completed sprint{'s' if completed != 1 else ''})  ·  "
        f"Remaining: {remaining} pts  ·  Estimated to complete: {est_text}{est_weeks}  ·  Generated: {generated_at[:10]}"
    )
    sc.font      = Font(italic=True, color=COLOUR_WHITE_FG, size=9)
    sc.fill      = _fill(COLOUR_HEADER_BG)
    sc.alignment = Alignment(horizontal="left", vertical="center")

    ws.row_dimensions[3].height = 20
    headers = ["Sprint", "Start", "End", "Planned Pts", "Delivered Pts",
               "Velocity (avg)", "Remaining (cum.)", "Status"]
    for col, h in enumerate(headers, start=1):
        c = ws.cell(row=3, column=col, value=h)
        c.font      = _font(bold=True)
        c.fill      = _fill(COLOUR_HEADER_BG)
        c.alignment = Alignment(horizontal="center", vertical="center")

    total_delivered_all = sum(s.get("delivered_points", 0) for s in sprints)
    cumulative_remaining = velocity.get("remaining_points", 0) + total_delivered_all
    row = 4

    for s in sprints:
        is_past    = s.get("is_past", False)
        is_current = s.get("is_current", False)
        if is_past:
            status, row_bg = "completed", "FFEBF5EB"
        elif is_current:
            status, row_bg = "active",    "FFFFF3CD"
        else:
            status, row_bg = "planned",   "FFFFFFFF"

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
            status,
        ]
        for col, val in enumerate(row_data, start=1):
            c = ws.cell(row=row, column=col, value=val)
            c.font      = Font(color=COLOUR_DARK_FG, size=10)
            c.fill      = PatternFill(fill_type="solid", fgColor=row_bg[2:])
            c.alignment = Alignment(horizontal="left" if col == 1 else "center", vertical="center")
            if col in (4, 5, 6, 7) and val is not None:
                c.number_format = "0"
        row += 1

    if est and avg > 0 and sprints:
        last_sprint  = sprints[-1]
        last_end     = datetime.strptime(last_sprint["end_date"], "%Y-%m-%d").date()
        sprint_days  = dur * 7
        proj_remaining = cumulative_remaining
        sprint_num   = 1

        ws.row_dimensions[row].height = 4
        row += 1

        ws.merge_cells(f"A{row}:H{row}")
        ws.row_dimensions[row].height = 17
        c = ws.cell(row=row, column=1, value="▸ Projected future sprints (based on current velocity)")
        c.font      = Font(bold=True, italic=True, color="FF444444", size=9)
        c.alignment = Alignment(horizontal="left", vertical="center")
        row += 1

        while proj_remaining > 0 and sprint_num <= 40:
            proj_start        = last_end + timedelta(days=1 + (sprint_num - 1) * sprint_days)
            proj_end          = proj_start + timedelta(days=sprint_days - 1)
            projected_delivery = min(avg, proj_remaining)
            proj_remaining    -= projected_delivery
            proj_remaining     = max(0, proj_remaining)

            ws.row_dimensions[row].height = 16
            row_data = [
                f"S{len(sprints) + sprint_num:03d}.projected",
                proj_start.strftime("%Y-%m-%d"),
                proj_end.strftime("%Y-%m-%d"),
                round(avg),
                round(projected_delivery),
                avg,
                max(0, proj_remaining),
                "projected",
            ]
            for col, val in enumerate(row_data, start=1):
                c = ws.cell(row=row, column=col, value=val)
                c.font      = Font(color="FF888888", italic=True, size=9)
                c.fill      = PatternFill(fill_type="solid", fgColor="FFF5F5F5")
                c.alignment = Alignment(horizontal="left" if col == 1 else "center", vertical="center")
                if col in (4, 5, 6, 7):
                    c.number_format = "0"
            row     += 1
            sprint_num += 1


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Generate WBS xlsx report from kanban JSON data (read from stdin)."
    )
    parser.add_argument("--template", required=True, help="Path to the WBS Excel template (.xlsx).")
    parser.add_argument("--output",   required=True, help="Output path for the generated report (.xlsx).")
    args = parser.parse_args()

    template_path = Path(args.template)
    output_path   = Path(args.output)

    if not template_path.exists():
        print(f"ERROR: Template not found: {template_path}", file=sys.stderr)
        sys.exit(1)

    try:
        raw      = sys.stdin.read()
        envelope = json.loads(raw)
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
    generated_at = data["generated_at"]

    stories_by_id = {s["id"]: s for s in stories}
    hierarchy     = _build_hierarchy(stories)
    sprint_dur    = velocity.get("sprint_duration_weeks", 2)
    estimates, hours_per_point = _compute_estimates(stories, velocity, sprint_dur)

    print(f"Loading template: {template_path}", file=sys.stderr)
    wb_template = openpyxl.load_workbook(str(template_path))
    col_widths, tdata = _extract_template_data(wb_template)

    wb_out = openpyxl.Workbook()

    # ── Sheet 1: WBS ──────────────────────────────────────────────────────────
    ws_src = wb_template["WBS – AutoPASS IP 2.0"]
    ws_wbs = wb_out.active
    ws_wbs.title = "WBS – AutoPASS IP 2.0"

    print("Building WBS sheet …", file=sys.stderr)
    build_wbs_sheet(ws_src, ws_wbs, stories_by_id, hierarchy,
                    estimates, hours_per_point, tdata, col_widths, generated_at)

    # ── Sheet 2: Phase Summary ────────────────────────────────────────────────
    ws_summary = wb_out.create_sheet("Phase Summary")
    ws_summary.sheet_view.showGridLines = False
    print("Building Phase Summary sheet …", file=sys.stderr)
    build_phase_summary_sheet(ws_summary, phases, stories)

    # ── Sheet 3: Sprint Burndown ──────────────────────────────────────────────
    ws_burndown = wb_out.create_sheet("Sprint Burndown")
    ws_burndown.sheet_view.showGridLines = False
    print("Building Sprint Burndown sheet …", file=sys.stderr)
    build_sprint_burndown_sheet(ws_burndown, sprints, velocity, generated_at)

    # ── Legend: copy from template if present ─────────────────────────────────
    if "Legend & Guide" in wb_template.sheetnames:
        ws_legend_src = wb_template["Legend & Guide"]
        ws_legend     = wb_out.create_sheet("Legend & Guide")
        for row in ws_legend_src.iter_rows():
            for cell in row:
                nc = ws_legend.cell(row=cell.row, column=cell.column, value=cell.value)
                if cell.has_style:
                    nc.font      = _copy.copy(cell.font)
                    nc.fill      = _copy.copy(cell.fill)
                    nc.alignment = _copy.copy(cell.alignment)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    wb_out.save(str(output_path))
    print(f"Report saved: {output_path}", file=sys.stderr)
    print(f"  Stories: {len(stories)}", file=sys.stderr)
    print(f"  Sprints: {len(sprints)}", file=sys.stderr)
    print(f"  Phases:  {len(phases)}", file=sys.stderr)
    if hours_per_point > 0:
        print(f"  Hours/point: {hours_per_point:.1f}h  (sprint={sprint_dur}w, velocity={velocity.get('avg_points_per_sprint', 0):.1f} pts/sprint)", file=sys.stderr)
    if velocity.get("estimated_sprints_remaining"):
        est_s = velocity["estimated_sprints_remaining"]
        print(f"  Prognosis: {est_s:.1f} sprints remaining (avg {velocity['avg_points_per_sprint']:.1f} pts/sprint)", file=sys.stderr)


if __name__ == "__main__":
    main()
