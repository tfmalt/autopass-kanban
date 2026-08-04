---
id: EP-005
type: epic
status: done
phase: 1
owner: Thomas Malt / Tooling Lead
milestone: MP2
created: 2026-08-04T12:00:00+0200
updated: 2026-08-04T13:58:18+0200
work_started: 2026-08-04T13:50:22+0200
work_done: 2026-08-04T13:58:18+0200
---

# Epic: Backlog reporting and export

---

---

## Business Context

The kanban tool already derives WBS and sprint information from the markdown
backlog and exports it to Excel. The report currently shows completion dates but
does not identify the sprint in which an artifact was completed, making sprint
delivery history harder to review from the WBS workbook. This Epic adds focused,
source-derived reporting improvements without introducing a second state store.

---

## Business Value

- **Primary benefit:** Project and tooling leads can identify the sprint that completed each story or epic directly in the WBS workbook.
- **Secondary benefit:** Report output remains reproducible from lifecycle timestamps, sprint metadata, and markdown source files.
- **Risk if not done:** Completion history must be reconstructed manually from sprint summaries and git history, increasing the risk of incorrect delivery reporting.

---

## Users and Stakeholders

| Role | Involvement |
| --- | --- |
| Project manager / tooling lead | Uses the Excel WBS report to review delivery history |
| Developer | Implements and tests report derivation in the shared core and workbook generator |
| Backlog maintainer | Maintains markdown lifecycle and sprint metadata used by the report |
| Product owner | Verifies that completion-sprint reporting is understandable and accurate |

---

## Scope

### In Scope

- Completion-sprint data in the precomputed WBS workbook report rows.
- A `Completed In Sprint` column in the generated Excel WBS sheet.
- User-story resolution from retained sprint membership for terminal stories.
- Epic resolution from the sprint date range containing the epic `work_done` timestamp.
- Explicit blank and warning/note behavior for rows where completion-sprint data cannot be resolved.
- Unit, workbook, and report-pipeline verification.

### Out of Scope

- Replacing markdown as the backlog source of truth.
- Adding a database, generated state store, or Excel-based completion history.
- Changing sprint rollover semantics or lifecycle timestamp semantics.
- Exposing the field in the web WBS report unless a separate story explicitly requires it.
- Reconstructing historical completion data from git history.

---

## Acceptance Criteria

- [ ] The WBS Excel report contains a `Completed In Sprint` column with documented semantics.
- [ ] The report model derives completion-sprint values before workbook generation, and the Python workbook script renders the precomputed value without reimplementing domain rules.
- [ ] The completion-sprint derivation is covered by automated tests for stories, epics, incomplete rows, and unresolved data.
- [ ] The report pipeline remains backward-compatible with backlog rows that do not provide enough information to resolve a completion sprint.
- [ ] The full verification suite and a generated workbook inspection pass before the Epic is accepted.

---

## Non-Functional Requirements

| Area | Requirement |
| --- | --- |
| **Correctness** | Completion-sprint values must be derived deterministically from the markdown-backed lifecycle and sprint data. |
| **Traceability** | The implementation must preserve the distinction between current sprint membership and historical completion sprint. |
| **Backward compatibility** | Existing WBS JSON consumers and workbook rows remain readable; unresolved values are blank rather than fabricated. |
| **Maintainability** | Report semantics remain in the Rust core model; the Python script owns workbook layout and presentation only. |
| **Usability** | The column and Legend & Guide text clearly explain story, epic, group-row, and missing-data behavior. |

---

## Architecture Considerations

- **Relevant architecture principles:** Markdown remains the only persisted source of truth; generated Excel files are outputs only.
- **Key patterns in play:** Precomputed report DTOs, explicit date-range resolution, and presentation-only workbook generation.
- **ADR references:** None required unless implementation introduces a new persisted lifecycle field or changes sprint rollover semantics.
- **Known risks or constraints:** Stories retain their assigned sprint when completed, but epics do not have sprint frontmatter. Epic completion therefore requires matching `work_done` to a sprint date range. Boundary and missing-data behavior must be explicit.

---

## Dependencies

| Dependency | Type | Status | Notes |
| --- | --- | --- | --- |
| Existing WBS report DTOs | Component | Available | `crates/core/src/json/report.rs` provides workbook rows |
| Existing sprint metadata | Component | Available | `SprintOverview` provides sprint names and date ranges |
| Existing Excel generator | Component | Available | `scripts/wbs_report.py` owns workbook layout |
| US-047 | Story | Draft | Implements the first completion-sprint export capability |

---

## Child User Stories

| Story ID | Title | Status | Points |
| --- | --- | --- | --- |
| US-047 | Add completed-in-sprint column to WBS Excel report | Draft | 5 |

---

## Definition of Done (Epic Level)

- [ ] All child User Stories are complete and accepted.
- [ ] Completion-sprint semantics are documented in the report guide.
- [ ] Rust and Python tests cover normal, boundary, and missing-data cases.
- [ ] The generated workbook has been opened or inspected for correct column placement and values.
- [ ] The standard verification suite passes.

---

## Notes and Open Questions

| # | Question / Assumption | Owner | Due | Resolved |
| --- | --- | --- | --- | --- |
| 1 | A story's retained `sprint` is the authoritative completion sprint when its status is `done` or `dropped`. | Tooling lead | 2026-08-04 | Yes |
| 2 | An epic's completion sprint is the sprint whose inclusive date range contains `work_done`; an unresolved match remains blank and is noted. | Tooling lead | 2026-08-04 | Yes |
| 3 | Phase and group rows do not receive an aggregated completion sprint. | Tooling lead | 2026-08-04 | Yes |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic Epic template derived from the kanban tooling conventions_
