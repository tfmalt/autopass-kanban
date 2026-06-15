#!/usr/bin/env python3

import unittest
from datetime import date
from pathlib import Path
import sys

import openpyxl

sys.path.insert(0, str(Path(__file__).resolve().parent))
import wbs_report


class WbsReportTests(unittest.TestCase):
    def test_aggregate_status_collapses_story_workflow_states(self):
        self.assertEqual(wbs_report._aggregate_status([]), "PLANNED")
        self.assertEqual(wbs_report._aggregate_status([{"status": "ready"}]), "PLANNED")
        self.assertEqual(wbs_report._aggregate_status([{"status": "todo"}]), "TODO")
        self.assertEqual(wbs_report._aggregate_status([{"status": "blocked"}]), "IN PROGRESS")
        self.assertEqual(wbs_report._aggregate_status([{"status": "done"}]), "DONE")
        self.assertEqual(
            wbs_report._aggregate_status([{"status": "done"}, {"status": "ready"}]),
            "IN PROGRESS",
        )

    def test_wbs_sheet_renders_planned_and_actual_date_columns(self):
        wb = openpyxl.Workbook()
        ws = wb.active
        hierarchy = [
            {
                "id": "F1",
                "epics": [
                    {
                        "id": "EP-F1-06",
                        "title": "Git-driven kanban and backlog tooling",
                        "stories": [
                            {
                                "id": "US-F1-058",
                                "title": "Add planned and actual dates",
                                "status": "done",
                                "story_points": 1,
                                "phase": "F1",
                                "epic_id": "EP-F1-06",
                                "planned_start": "2026-06-15",
                                "planned_end": "2026-06-19",
                                "work_started": "2026-06-17T09:00:00+0200",
                                "work_done": "2026-07-01T16:00:00+0200",
                            },
                            {
                                "id": "US-F1-059",
                                "title": "Missing planned dates stay visible",
                                "status": "todo",
                                "story_points": 2,
                                "phase": "F1",
                                "epic_id": "EP-F1-06",
                            },
                        ],
                    }
                ],
            }
        ]
        estimates = {
            "US-F1-058": {"est_hours": None, "est_start": None, "est_end": None},
            "US-F1-059": {"est_hours": 14, "est_start": date(2026, 6, 22), "est_end": date(2026, 6, 23)},
        }

        wbs_report.build_wbs_sheet(ws, hierarchy, estimates, 7.0, "2026-06-11T10:00:00+02:00")

        headers = [ws.cell(2, col).value for col in range(1, wbs_report.TOTAL_COLS + 1)]
        self.assertIn("Planned Start Date", headers)
        self.assertIn("Planned End Date", headers)
        self.assertIn("Actual Start Date", headers)
        self.assertIn("Actual End Date", headers)
        self.assertIn("Actual Period", headers)

        self.assertEqual(ws.cell(5, wbs_report.COL_PERIOD).value, "Q2 2026")
        self.assertEqual(ws.cell(5, wbs_report.COL_PLANNED_START_DATE).value, date(2026, 6, 15))
        self.assertEqual(ws.cell(5, wbs_report.COL_PLANNED_END_DATE).value, date(2026, 6, 19))
        self.assertEqual(ws.cell(5, wbs_report.COL_ACTUAL_START_DATE).value, date(2026, 6, 17))
        self.assertEqual(ws.cell(5, wbs_report.COL_ACTUAL_END_DATE).value, date(2026, 7, 1))
        self.assertEqual(ws.cell(5, wbs_report.COL_ACTUAL_PERIOD).value, "Q2-Q3 2026")

        self.assertIsNone(ws.cell(6, wbs_report.COL_PERIOD).value)
        self.assertIsNone(ws.cell(6, wbs_report.COL_PLANNED_START_DATE).value)
        self.assertIsNone(ws.cell(6, wbs_report.COL_PLANNED_END_DATE).value)
        self.assertIsNone(ws.cell(6, wbs_report.COL_ACTUAL_PERIOD).value)
        self.assertEqual(
            ws.cell(6, wbs_report.COL_NOTES).value,
            "Missing planned baseline: start, end",
        )

        self.assertFalse(ws.sheet_properties.outlinePr.summaryBelow)
        self.assertEqual(ws.row_dimensions[3].outlineLevel, 0)
        self.assertEqual(ws.row_dimensions[4].outlineLevel, 1)
        self.assertEqual(ws.row_dimensions[5].outlineLevel, 2)
        self.assertEqual(ws.row_dimensions[6].outlineLevel, 2)

    def test_wbs_sheet_rolls_story_status_up_to_epic_and_phase(self):
        wb = openpyxl.Workbook()
        ws = wb.active
        hierarchy = [
            {
                "id": "F1",
                "epics": [
                    {
                        "id": "EP-F1-01",
                        "title": "Active epic",
                        "stories": [
                            {
                                "id": "US-F1-001",
                                "title": "Started story",
                                "status": "in-progress",
                                "story_points": 1,
                                "phase": "F1",
                                "epic_id": "EP-F1-01",
                            },
                            {
                                "id": "US-F1-002",
                                "title": "Queued story",
                                "status": "todo",
                                "story_points": 1,
                                "phase": "F1",
                                "epic_id": "EP-F1-01",
                            },
                        ],
                    },
                    {
                        "id": "EP-F1-02",
                        "title": "Completed epic",
                        "stories": [
                            {
                                "id": "US-F1-003",
                                "title": "Finished story",
                                "status": "done",
                                "story_points": 1,
                                "phase": "F1",
                                "epic_id": "EP-F1-02",
                            },
                        ],
                    },
                ],
            }
        ]
        estimates = {
            story["id"]: {"est_hours": None, "est_start": None, "est_end": None}
            for epic in hierarchy[0]["epics"]
            for story in epic["stories"]
        }

        wbs_report.build_wbs_sheet(ws, hierarchy, estimates, 0.0, "2026-06-12T10:00:00+02:00")

        self.assertEqual(ws.cell(3, wbs_report.COL_STATUS).value, "IN PROGRESS")
        self.assertEqual(ws.cell(4, wbs_report.COL_STATUS).value, "IN PROGRESS")
        self.assertEqual(ws.cell(7, wbs_report.COL_STATUS).value, "DONE")


if __name__ == "__main__":
    unittest.main()
