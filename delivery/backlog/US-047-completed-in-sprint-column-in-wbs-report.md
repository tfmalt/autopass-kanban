---
id: US-047
type: user-story
status: done
epic: EP-005
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-08-04T13:50:22+0200
work_done: 2026-08-04T13:58:18+0200
created: 2026-08-04T12:00:00+0200
updated: 2026-08-04T13:58:18+0200
---

# User Story: Add completed-in-sprint column to WBS Excel report

---

## Story Statement

**As a** project manager,
**I want** the WBS Excel report to show the sprint in which each completed story
or epic was completed,
**so that** I can review delivery history without manually correlating the WBS
with sprint summaries.

---

## Background

The WBS report already receives precomputed workbook rows from the Rust report
model and renders them through `scripts/wbs_report.py`. It currently includes
status and actual completion dates but no dedicated completion-sprint column.
Stories retain their sprint assignment when a sprint closes, while unfinished
stories are carried to the next sprint. Epics have completion timestamps but no
sprint frontmatter, so their completion sprint must be resolved from the sprint
date range containing `work_done`.

---

## Acceptance Criteria

**Scenario 1: The WBS sheet exposes completion sprint**

```gherkin
Given the WBS report is generated from valid kanban report JSON
When the workbook is opened
Then the WBS sheet contains a column named "Completed In Sprint"
And the Legend & Guide documents the meaning of the column
```

**Scenario 2: A completed story uses its retained sprint**

```gherkin
Given a story has status "done" and sprint "S001.foundation"
And the story has a valid work_done timestamp
When the WBS report model is built
Then the story workbook row contains "S001.foundation" as completed_in_sprint
And the Excel cell in "Completed In Sprint" contains "S001.foundation"
```

**Scenario 3: A dropped story is treated as terminal without changing its sprint**

```gherkin
Given a story has status "dropped" and sprint "S002.delivery"
When the WBS report model is built
Then the story workbook row contains "S002.delivery" as completed_in_sprint
```

**Scenario 4: An epic is resolved from its completion date**

```gherkin
Given an epic has a valid work_done date within the inclusive range of sprint "S001.foundation"
And the epic has no sprint frontmatter field
When the WBS report model is built
Then the epic workbook row contains "S001.foundation" as completed_in_sprint
```

**Scenario 5: Incomplete and group rows remain blank**

```gherkin
Given a story is not terminal
And a phase row and an epic group row are present in the WBS hierarchy
When the WBS report is generated
Then the incomplete story, phase row, and epic group row have blank Completed In Sprint cells
```

**Scenario 6: Unresolved data fails safely**

```gherkin
Given a terminal story has no sprint
Or an epic work_done date does not fall within any known sprint
When the WBS report is generated
Then the Completed In Sprint cell is blank
And the report records a clear note or warning
And report generation succeeds without inventing a sprint name
```

**Scenario 7: Sprint boundaries are deterministic**

```gherkin
Given an epic work_done date is exactly on a sprint start or end date
When the completion sprint is resolved
Then the documented inclusive boundary rule is applied consistently
And automated tests verify the result
```

---

## Non-Functional Requirements

| Area | Requirement |
| --- | --- |
| **Correctness** | Completion sprint values are deterministic and use retained story sprint membership or epic completion-date matching as specified. |
| **Traceability** | The report must not confuse an item's current sprint with the sprint in which it completed. |
| **Backward compatibility** | Missing completion-sprint inputs leave cells blank and do not break existing report generation or JSON consumers. |
| **Maintainability** | Completion-sprint semantics are implemented in the Rust report model; the Python generator only writes the DTO field. |
| **Usability** | Column placement, header, widths, and guide text make the new value easy to understand in the workbook. |

---

## Technical Notes

- **Requirement refs:** Internal tooling requirement; no AutoPASS product requirement applies.
- **Acceptance criteria refs:** `EP-005#acceptance-criteria`, `US-047` scenarios 1–7.
- **Scenarios:** Completion-sprint reporting for terminal stories and epics.
- **Feature tokens:** N/A — internal kanban reporting capability.
- **Component / Module:** `crates/core/src/json/report.rs`, `crates/core/src/model.rs` as needed, `scripts/wbs_report.py`, and their test modules.
- **Key integration points:** `kanban --format json report wbs`, `ReportWorkbookRowDto`, `wbs_rows`, workbook column layout, and `Legend & Guide`.
- **Suggested patterns:** Add an optional `completed_in_sprint` field to `ReportWorkbookRowDto`; resolve story values from terminal story sprint membership; resolve epic values by parsing sprint date ranges and matching the epic `work_done` date; keep group-row policy explicit.
- **Data model hints:** Epic metadata may need to be loaded alongside the existing story hierarchy because current WBS grouping resolves epic identity from child stories but does not currently project epic lifecycle fields.
- **Testing approach:** Add Rust unit tests for story and epic resolution, inclusive sprint boundaries, and unresolved values; update Python workbook tests for headers, cell placement, blank handling, and legend text; run the complete report pipeline and inspect the generated workbook.
- **Migration / backward compatibility:** Do not add persisted `completed_in_sprint` frontmatter. Existing stories and epics without resolvable data remain reportable with blank cells and visible notes or warnings.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` are set on authoring; `work_started` is set on the first move to `in-progress`.

---

## Definition of Done

- [x] `ReportWorkbookRowDto` exposes an optional completion-sprint value.
- [x] Terminal story rows use their retained sprint assignment.
- [x] Epic rows resolve completion sprint from `work_done` and sprint date ranges.
- [x] Phase and group-row behavior is implemented and documented.
- [x] Missing or inconsistent data remains blank and is visible through notes or warnings.
- [x] Excel WBS headers, widths, writers, style ranges, and Legend & Guide are updated.
- [x] Rust unit tests and Python workbook tests cover all acceptance scenarios.
- [x] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass.
- [x] `kanban validate .` and `kanban doctor .` pass.
- [x] A generated workbook is inspected for correct values and column placement.
- [x] Story is demo-ready for sprint review.

---

## Dependencies

| Dependency | Type | Status | Notes |
| --- | --- | --- | --- |
| EP-005 | Epic | Draft | Owns backlog reporting and export improvements |
| `ReportWorkbookRowDto` | Component | Available | Existing precomputed workbook-row DTO |
| `SprintOverview` | Component | Available | Provides sprint names and date ranges |
| `scripts/wbs_report.py` | Component | Available | Existing Excel workbook generator |

---

## Sprint Task Log Guidance

When this story is activated, track implementation tasks in a sibling
`US-047-completed-in-sprint-column-in-wbs-report.tasks.md` file. Keep tasks
focused on report-model derivation, epic lookup and date matching, workbook
layout, documentation, and verification.

---

## Notes and Open Questions

| # | Question / Assumption | Owner | Due | Resolved |
| --- | --- | --- | --- | --- |
| 1 | A terminal story's retained `sprint` is authoritative for completion reporting, including `dropped` stories. | Tooling lead | 2026-08-04 | Yes |
| 2 | Epic completion uses the sprint whose date range includes `work_done`; both start and end dates are inclusive. | Tooling lead | 2026-08-04 | Yes |
| 3 | Phase and epic group rows have no single completion sprint and remain blank. | Tooling lead | 2026-08-04 | Yes |
| 4 | The web report remains unchanged unless a follow-up story expands the field beyond Excel. | Tooling lead | 2026-08-04 | Yes |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
