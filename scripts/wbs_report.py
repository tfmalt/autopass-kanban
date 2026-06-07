#!/usr/bin/env python3
"""
Generate an updated WBS Excel report from AutoPASS IP 2.0 kanban data.

Usage:
    kanban --format json report wbs | python3 tools/kanban/scripts/wbs_report.py \\
        --template delivery/backlog/2026-03-31.autopass_ip_2.0_wbs.xlsx \\
        --output delivery/backlog/wbs_report.xlsx

The script reads JSON from stdin (produced by `kanban --format json report wbs`),
then reads the WBS template for style/layout reference, and writes an updated xlsx
with current story statuses, story points, and a sprint burndown prognosis sheet.
"""

import argparse
import json
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


# ── Colour palette matching the original WBS template ────────────────────────

COLOUR_TITLE_BG = "FF0D1F40"   # dark navy — title row background
COLOUR_HEADER_BG = "FF1A3060"  # dark blue — column header row
COLOUR_PHASE_BG = "FF1F3864"   # navy — level-1 (phase) rows
COLOUR_EPIC_BG = "FF2E5EAA"    # medium blue — level-2 (epic) rows
COLOUR_STORY_BG = "FFD6E4F0"   # light blue — level-3 (story) rows
COLOUR_TASK_BG = "00000000"    # transparent — level-4 (task) rows
COLOUR_WHITE_FG = "FFFFFFFF"
COLOUR_DARK_FG = "FF1F1F1F"

STATUS_MAP = {
    "draft": "DRAFT",
    "ready": "READY",
    "todo": "TODO",
    "in-progress": "IN PROGRESS",
    "ready-for-qa": "READY FOR QA",
    "blocked": "BLOCKED",
    "done": "DONE",
    "dropped": "DROPPED",
}

PHASE_META = {
    "F1": {
        "title": "Phase 1 – Etablering (Establishment)",
        "milestone": "MP1 – Foundation",
        "period": "Q2 2026",
        "priority": "Critical",
    },
    "F2": {
        "title": "Phase 2 – Utvikling: Kjernelogikk (Core Logic Development)",
        "milestone": "MP2 – Core Logic",
        "period": "Q3 2026",
        "priority": "Critical",
    },
    "F3": {
        "title": "Phase 3 – Utvikling: Administrasjon (Administration Development)",
        "milestone": "MP3 – Administration",
        "period": "Q4 2026",
        "priority": "High",
    },
    "F4": {
        "title": "Phase 4 – Utvikling: Ferdigstillelse (Completion)",
        "milestone": "MP4 – Complete Functionality",
        "period": "Q1 2027",
        "priority": "High",
    },
    "F5": {
        "title": "Phase 5 – Driftssettelse og Stabilisering (Go-Live & Stabilization)",
        "milestone": "MP5 – Production Readiness",
        "period": "Q2 2027",
        "priority": "High",
    },
}


# ── Style helpers ─────────────────────────────────────────────────────────────

def fill(hex_colour: str) -> PatternFill:
    if hex_colour == COLOUR_TASK_BG:
        return PatternFill(fill_type=None)
    return PatternFill(fill_type="solid", fgColor=hex_colour)


def font(bold: bool = False, colour: str = COLOUR_WHITE_FG, size: int = 10) -> Font:
    return Font(bold=bold, color=colour, size=size)


def thin_border() -> Border:
    side = Side(style="thin", color="FFB0B0B0")
    return Border(bottom=side)


def apply_row_style(ws, row_num: int, level: int, col_count: int = 10):
    """Apply background + font colour for a WBS hierarchy level."""
    if level == 0:
        bg, fg, bold = COLOUR_TITLE_BG, COLOUR_WHITE_FG, True
    elif level == 1:
        bg, fg, bold = COLOUR_HEADER_BG, COLOUR_WHITE_FG, True
    elif level == 2:
        bg, fg, bold = COLOUR_PHASE_BG, COLOUR_WHITE_FG, True
    elif level == 3:
        bg, fg, bold = COLOUR_EPIC_BG, COLOUR_WHITE_FG, True
    elif level == 4:
        bg, fg, bold = COLOUR_STORY_BG, COLOUR_DARK_FG, False
    else:
        bg, fg, bold = COLOUR_TASK_BG, COLOUR_DARK_FG, False

    for col in range(1, col_count + 1):
        cell = ws.cell(row=row_num, column=col)
        if bg != COLOUR_TASK_BG:
            cell.fill = fill(bg)
        cell.font = font(bold=bold, colour=fg)
        cell.alignment = Alignment(vertical="center", wrap_text=False)


# ── Template column layout ────────────────────────────────────────────────────
# A=WBS No, B=ID, C=Title, D=Milestone, E=Period, F=Priority, G=Status,
# H=Story Pts, I=Est Hours, J=Notes
COL_WBS = 1
COL_ID = 2
COL_TITLE = 3
COL_MILESTONE = 4
COL_PERIOD = 5
COL_PRIORITY = 6
COL_STATUS = 7
COL_POINTS = 8
COL_HOURS = 9
COL_NOTES = 10


# ── WBS sheet builder ─────────────────────────────────────────────────────────

def build_wbs_sheet(ws_src, ws_dst, stories_by_id: dict, epics_by_phase: dict):
    """
    Copy the source WBS sheet to ws_dst, updating Status and Story Pts columns
    from live kanban data. Appends rows for stories not in the template.
    """
    # Copy column dimensions
    for col_letter, dim in ws_src.column_dimensions.items():
        ws_dst.column_dimensions[col_letter].width = dim.width

    # Track which story IDs we've seen in the template
    seen_story_ids = set()
    # Track which epic sections exist per phase (for appending missing stories)
    epic_row_end: dict = {}  # epic_id → last row in that epic section
    current_phase = None
    current_epic = None
    row_dst = 1

    for row_src in ws_src.iter_rows(min_row=1):
        values = [c.value for c in row_src]
        src_id = values[COL_ID - 1] if len(values) >= COL_ID else None
        src_row_num = row_src[0].row

        # Copy row height
        src_height = ws_src.row_dimensions[src_row_num].height
        if src_height:
            ws_dst.row_dimensions[row_dst].height = src_height

        # Detect row type by ID prefix
        is_phase_row = src_id and not str(src_id).startswith(("EP-", "US-", "T-"))
        is_epic_row = src_id and str(src_id).startswith("EP-")
        is_story_row = src_id and str(src_id).startswith("US-")

        if is_phase_row:
            current_phase = src_id
        elif is_epic_row:
            current_epic = src_id

        # Determine effective values
        effective_values = list(values[:10])  # copy first 10 columns

        if is_story_row:
            seen_story_ids.add(str(src_id))
            live = stories_by_id.get(str(src_id))
            if live:
                effective_values[COL_STATUS - 1] = STATUS_MAP.get(
                    live["status"].lower(), live["status"].upper()
                )
                effective_values[COL_POINTS - 1] = live.get("story_points")
                if live.get("sprint"):
                    effective_values[COL_NOTES - 1] = (
                        effective_values[COL_NOTES - 1] or ""
                    )  # keep existing notes

        # Write cells
        for col_idx, val in enumerate(effective_values, start=1):
            ws_dst.cell(row=row_dst, column=col_idx, value=val)

        # Copy source cell styles (fill, font, alignment) from template
        for cell_src in row_src[:10]:
            cell_dst = ws_dst.cell(row=row_dst, column=cell_src.column)
            if cell_src.has_style:
                cell_dst.font = _copy.copy(cell_src.font)
                cell_dst.fill = _copy.copy(cell_src.fill)
                cell_dst.alignment = _copy.copy(cell_src.alignment)
                cell_dst.border = _copy.copy(cell_src.border)

        # Update status/points cells with correct style (override template defaults)
        if is_story_row:
            live = stories_by_id.get(str(src_id))
            if live:
                # Re-apply story row style so font colour is correct
                apply_row_style(ws_dst, row_dst, level=4)

        if is_epic_row:
            epic_row_end[str(src_id)] = row_dst
        elif is_story_row and current_epic:
            epic_row_end[current_epic] = row_dst

        row_dst += 1

    # Append stories not in template
    missing_stories = [
        s for sid, s in stories_by_id.items() if sid not in seen_story_ids
    ]
    if missing_stories:
        # Group missing stories by phase/epic
        missing_by_phase: dict = {}
        for s in missing_stories:
            ph = s["phase"]
            epic = s.get("epic_id") or f"EP-{ph}-??"
            missing_by_phase.setdefault(ph, {}).setdefault(epic, []).append(s)

        for phase_id in sorted(missing_by_phase):
            phase_meta = PHASE_META.get(phase_id, {})
            ws_dst.row_dimensions[row_dst].height = 19.5
            ws_dst.cell(row=row_dst, column=COL_WBS, value="(new)")
            ws_dst.cell(row=row_dst, column=COL_ID, value=phase_id)
            ws_dst.cell(
                row=row_dst,
                column=COL_TITLE,
                value=f"[New stories added after WBS baseline – {phase_meta.get('title', phase_id)}]",
            )
            ws_dst.cell(row=row_dst, column=COL_MILESTONE, value=phase_meta.get("milestone", ""))
            ws_dst.cell(row=row_dst, column=COL_PERIOD, value=phase_meta.get("period", ""))
            apply_row_style(ws_dst, row_dst, level=2)
            row_dst += 1

            for epic_id in sorted(missing_by_phase[phase_id]):
                epic_stories = missing_by_phase[phase_id][epic_id]
                ws_dst.row_dimensions[row_dst].height = 18.0
                ws_dst.cell(row=row_dst, column=COL_WBS, value="(new)")
                ws_dst.cell(row=row_dst, column=COL_ID, value=epic_id)
                epic_title = next(
                    (s.get("epic_title") for s in epic_stories if s.get("epic_title")), epic_id
                )
                ws_dst.cell(row=row_dst, column=COL_TITLE, value=f"   {epic_title}")
                ws_dst.cell(row=row_dst, column=COL_MILESTONE, value=phase_meta.get("milestone", ""))
                ws_dst.cell(row=row_dst, column=COL_PERIOD, value=phase_meta.get("period", ""))
                apply_row_style(ws_dst, row_dst, level=3)
                row_dst += 1

                for i, s in enumerate(sorted(epic_stories, key=lambda x: x["id"]), start=1):
                    ws_dst.row_dimensions[row_dst].height = 18.0
                    ws_dst.cell(row=row_dst, column=COL_WBS, value="(new)")
                    ws_dst.cell(row=row_dst, column=COL_ID, value=s["id"])
                    ws_dst.cell(row=row_dst, column=COL_TITLE, value=f"      {s['title']}")
                    ws_dst.cell(row=row_dst, column=COL_MILESTONE, value=phase_meta.get("milestone", ""))
                    ws_dst.cell(row=row_dst, column=COL_PERIOD, value=phase_meta.get("period", ""))
                    ws_dst.cell(
                        row=row_dst,
                        column=COL_STATUS,
                        value=STATUS_MAP.get(s["status"].lower(), s["status"].upper()),
                    )
                    ws_dst.cell(row=row_dst, column=COL_POINTS, value=s.get("story_points"))
                    apply_row_style(ws_dst, row_dst, level=4)
                    row_dst += 1

    # Merge title row A1:J1 if not already merged
    if row_dst > 1:
        try:
            ws_dst.merge_cells(start_row=1, start_column=1, end_row=1, end_column=10)
        except Exception:
            pass


# ── Phase Summary sheet builder ────────────────────────────────────────────────

def build_phase_summary_sheet(ws, phases: list, stories: list):
    """Rebuild the Phase Summary sheet from live data."""
    # Count epics per phase
    epics_by_phase: dict = {}
    for s in stories:
        ph = s["phase"]
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

    # Title row
    ws.row_dimensions[1].height = 28
    ws.merge_cells("A1:J1")
    title_cell = ws["A1"]
    title_cell.value = "AutoPASS IP 2.0 — Phase & Milestone Summary"
    title_cell.font = Font(bold=True, color=COLOUR_WHITE_FG, size=13)
    title_cell.fill = fill(COLOUR_TITLE_BG)
    title_cell.alignment = Alignment(horizontal="left", vertical="center")

    # Header row
    ws.row_dimensions[2].height = 20
    headers = ["Phase", "Title", "Period", "Milestone", "Epics", "Stories", "Pts Total", "Pts Done", "Pts In Progress", "Pts Remaining"]
    for col, h in enumerate(headers, start=1):
        cell = ws.cell(row=2, column=col, value=h)
        cell.font = font(bold=True)
        cell.fill = fill(COLOUR_HEADER_BG)
        cell.alignment = Alignment(horizontal="center", vertical="center")

    phase_totals = {"epics": 0, "stories": 0, "total": 0, "done": 0, "wip": 0, "remaining": 0}
    row = 3
    for ph_dto in sorted(phases, key=lambda p: p["phase"]):
        ph_id = ph_dto["phase"]
        meta = PHASE_META.get(ph_id, {})
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
            cell = ws.cell(row=row, column=col, value=val)
            cell.font = font(bold=False, colour=COLOUR_DARK_FG)
            cell.alignment = Alignment(horizontal="left" if col <= 4 else "center", vertical="center")
            if col >= 5:
                cell.fill = fill("FFE8F0FA")

        phase_totals["epics"] += epic_count
        phase_totals["stories"] += ph_dto["story_count"]
        phase_totals["total"] += ph_dto["points_total"]
        phase_totals["done"] += ph_dto["points_done"]
        phase_totals["wip"] += ph_dto["points_in_progress"]
        phase_totals["remaining"] += ph_dto["points_remaining"]
        row += 1

    # Totals row
    ws.row_dimensions[row].height = 20
    totals = ["TOTAL", "", "", "", phase_totals["epics"], phase_totals["stories"],
              phase_totals["total"], phase_totals["done"], phase_totals["wip"], phase_totals["remaining"]]
    for col, val in enumerate(totals, start=1):
        cell = ws.cell(row=row, column=col, value=val)
        cell.font = Font(bold=True, color=COLOUR_WHITE_FG, size=10)
        cell.fill = fill(COLOUR_PHASE_BG)
        cell.alignment = Alignment(horizontal="left" if col <= 4 else "center", vertical="center")


# ── Sprint Burndown sheet builder ─────────────────────────────────────────────

def build_sprint_burndown_sheet(ws, sprints: list, velocity: dict, generated_at: str):
    """Build a sprint-by-sprint burndown with velocity and prognosis."""
    ws.column_dimensions["A"].width = 28
    ws.column_dimensions["B"].width = 13
    ws.column_dimensions["C"].width = 13
    ws.column_dimensions["D"].width = 14
    ws.column_dimensions["E"].width = 14
    ws.column_dimensions["F"].width = 14
    ws.column_dimensions["G"].width = 16
    ws.column_dimensions["H"].width = 18

    # Title
    ws.merge_cells("A1:H1")
    ws.row_dimensions[1].height = 28
    title = ws["A1"]
    title.value = "AutoPASS IP 2.0 — Sprint Burndown & Prognosis"
    title.font = Font(bold=True, color=COLOUR_WHITE_FG, size=13)
    title.fill = fill(COLOUR_TITLE_BG)
    title.alignment = Alignment(horizontal="left", vertical="center")

    # Velocity summary band
    ws.merge_cells("A2:H2")
    ws.row_dimensions[2].height = 16
    avg = velocity.get("avg_points_per_sprint", 0)
    remaining = velocity.get("remaining_points", 0)
    est = velocity.get("estimated_sprints_remaining")
    completed = velocity.get("completed_sprint_count", 0)
    dur = velocity.get("sprint_duration_weeks", 2)

    est_text = f"{est:.1f} sprints" if est else "—"
    est_weeks = f" ({est * dur:.0f} weeks)" if est else ""
    summary_cell = ws["A2"]
    summary_cell.value = (
        f"Velocity: {avg:.1f} pts/sprint (over {completed} completed sprint{'s' if completed != 1 else ''})  ·  "
        f"Remaining: {remaining} pts  ·  Estimated to complete: {est_text}{est_weeks}  ·  Generated: {generated_at[:10]}"
    )
    summary_cell.font = Font(italic=True, color=COLOUR_WHITE_FG, size=9)
    summary_cell.fill = fill(COLOUR_HEADER_BG)
    summary_cell.alignment = Alignment(horizontal="left", vertical="center")

    # Header row
    ws.row_dimensions[3].height = 20
    headers = ["Sprint", "Start", "End", "Planned Pts", "Delivered Pts", "Velocity (avg)", "Remaining (cum.)", "Status"]
    for col, h in enumerate(headers, start=1):
        cell = ws.cell(row=3, column=col, value=h)
        cell.font = font(bold=True)
        cell.fill = fill(COLOUR_HEADER_BG)
        cell.alignment = Alignment(horizontal="center", vertical="center")

    today = date.today()
    # Start from total story points (remaining + all delivered across all sprints)
    total_delivered_all = sum(s.get("delivered_points", 0) for s in sprints)
    cumulative_remaining = velocity.get("remaining_points", 0) + total_delivered_all
    row = 4

    for s in sprints:
        end_date = datetime.strptime(s["end_date"], "%Y-%m-%d").date()
        is_past = s.get("is_past", False)
        is_current = s.get("is_current", False)

        if is_past:
            status = "completed"
            row_bg = "FFEBF5EB"
        elif is_current:
            status = "active"
            row_bg = "FFFFF3CD"
        else:
            status = "planned"
            row_bg = "FFFFFFFF"

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
            cell = ws.cell(row=row, column=col, value=val)
            cell.font = Font(color=COLOUR_DARK_FG, size=10)
            cell.fill = PatternFill(fill_type="solid", fgColor=row_bg[2:])
            cell.alignment = Alignment(horizontal="left" if col == 1 else "center", vertical="center")
            if col in (4, 5, 6, 7) and val is not None:
                cell.number_format = "0"

        row += 1

    # Prognosis rows (future sprints based on velocity)
    if est and avg > 0 and sprints:
        last_sprint = sprints[-1]
        last_end = datetime.strptime(last_sprint["end_date"], "%Y-%m-%d").date()
        sprint_days = dur * 7
        proj_remaining = cumulative_remaining
        sprint_num = 1

        ws.row_dimensions[row].height = 4
        row += 1  # spacer

        proj_header_row = row
        ws.merge_cells(f"A{row}:H{row}")
        ws.row_dimensions[row].height = 17
        cell = ws.cell(row=row, column=1, value="▸ Projected future sprints (based on current velocity)")
        cell.font = Font(bold=True, italic=True, color="FF444444", size=9)
        cell.alignment = Alignment(horizontal="left", vertical="center")
        row += 1

        while proj_remaining > 0 and sprint_num <= 40:
            proj_start = last_end + timedelta(days=1 + (sprint_num - 1) * sprint_days)
            proj_end = proj_start + timedelta(days=sprint_days - 1)
            projected_delivery = min(avg, proj_remaining)
            proj_remaining -= projected_delivery
            proj_remaining = max(0, proj_remaining)

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
                cell = ws.cell(row=row, column=col, value=val)
                cell.font = Font(color="FF888888", italic=True, size=9)
                cell.fill = PatternFill(fill_type="solid", fgColor="FFF5F5F5")
                cell.alignment = Alignment(horizontal="left" if col == 1 else "center", vertical="center")
                if col in (4, 5, 6, 7):
                    cell.number_format = "0"
            row += 1
            sprint_num += 1


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Generate WBS xlsx report from kanban JSON data (read from stdin)."
    )
    parser.add_argument(
        "--template",
        required=True,
        help="Path to the WBS Excel template (.xlsx).",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Output path for the generated report (.xlsx).",
    )
    args = parser.parse_args()

    template_path = Path(args.template)
    output_path = Path(args.output)

    if not template_path.exists():
        print(f"ERROR: Template not found: {template_path}", file=sys.stderr)
        sys.exit(1)

    # Read JSON from stdin
    try:
        raw = sys.stdin.read()
        envelope = json.loads(raw)
    except json.JSONDecodeError as e:
        print(f"ERROR: Failed to parse JSON from stdin: {e}", file=sys.stderr)
        sys.exit(1)

    if envelope.get("status") != "ok":
        msg = envelope.get("error", {}).get("message", "unknown error")
        print(f"ERROR: kanban reported an error: {msg}", file=sys.stderr)
        sys.exit(1)

    data = envelope["data"]
    stories = data["stories"]
    sprints = data["sprints"]
    phases = data["phases"]
    velocity = data["velocity"]
    generated_at = data["generated_at"]

    stories_by_id = {s["id"]: s for s in stories}

    # Build epic → stories mapping per phase
    epics_by_phase: dict = {}
    for s in stories:
        ph = s["phase"]
        epic = s.get("epic_id") or f"EP-{ph}-??"
        epics_by_phase.setdefault(ph, {}).setdefault(epic, []).append(s)

    print(f"Loading template: {template_path}", file=sys.stderr)
    wb_template = openpyxl.load_workbook(str(template_path))

    wb_out = openpyxl.Workbook()

    # ── Sheet 1: WBS ──────────────────────────────────────────────────────────
    ws_src = wb_template["WBS – AutoPASS IP 2.0"]
    ws_wbs = wb_out.active
    ws_wbs.title = "WBS – AutoPASS IP 2.0"
    ws_wbs.sheet_view.showGridLines = False

    print("Building WBS sheet ...", file=sys.stderr)
    build_wbs_sheet(ws_src, ws_wbs, stories_by_id, epics_by_phase)

    # ── Sheet 2: Phase Summary ────────────────────────────────────────────────
    ws_summary = wb_out.create_sheet("Phase Summary")
    ws_summary.sheet_view.showGridLines = False

    print("Building Phase Summary sheet ...", file=sys.stderr)
    build_phase_summary_sheet(ws_summary, phases, stories)

    # ── Sheet 3: Sprint Burndown ──────────────────────────────────────────────
    ws_burndown = wb_out.create_sheet("Sprint Burndown")
    ws_burndown.sheet_view.showGridLines = False

    print("Building Sprint Burndown sheet ...", file=sys.stderr)
    build_sprint_burndown_sheet(ws_burndown, sprints, velocity, generated_at)

    # ── Legend: copy from template if present ─────────────────────────────────
    if "Legend & Guide" in wb_template.sheetnames:
        ws_legend_src = wb_template["Legend & Guide"]
        ws_legend = wb_out.create_sheet("Legend & Guide")
        for row in ws_legend_src.iter_rows():
            for cell in row:
                new_cell = ws_legend.cell(row=cell.row, column=cell.column, value=cell.value)
                if cell.has_style:
                    new_cell.font = _copy.copy(cell.font)
                    new_cell.fill = _copy.copy(cell.fill)
                    new_cell.alignment = _copy.copy(cell.alignment)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    wb_out.save(str(output_path))
    print(f"Report saved: {output_path}", file=sys.stderr)
    print(f"  Stories: {len(stories)}", file=sys.stderr)
    print(f"  Sprints: {len(sprints)}", file=sys.stderr)
    print(f"  Phases:  {len(phases)}", file=sys.stderr)
    if velocity.get("estimated_sprints_remaining"):
        print(
            f"  Prognosis: {velocity['estimated_sprints_remaining']:.1f} sprints remaining "
            f"(avg {velocity['avg_points_per_sprint']:.1f} pts/sprint)",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
