---
id: EP-006
type: epic
status: done
phase: 1
owner: Thomas Malt / Tooling Lead
milestone: MP2
created: 2026-08-25T09:18:12+0200
updated: 2026-08-25T10:22:32+0200
work_started: 2026-08-25T09:19:43+0200
work_done: 2026-08-25T10:22:32+0200
---

# Epic: Web task management

---

## Business Context

Developers can inspect a story and update an existing task's status in the
local web UI, but must switch to the CLI to create, edit, reorder, or remove
tasks. The web UI should support the complete task-planning workflow while
keeping the markdown task log as the sole source of truth.

## Scope

### In Scope

- Creating tasks from a story detail view.
- Editing task title, description, tags, and status.
- Moving tasks earlier or later in their story task log.
- Deleting a task after explicit confirmation.
- Validated, locked, atomic task-log mutations through `kanban-core`.

### Out of Scope

- New task statuses, standalone task files, or a database-backed task store.
- Bulk task editing or drag-and-drop interactions outside a story detail view.

## Acceptance Criteria

- [ ] A developer can create, edit, order, and delete tasks while viewing a story.
- [ ] Each mutation updates only the story's canonical sibling `.tasks.md` file.
- [ ] Reordering rejects incomplete, duplicate, or unknown task-id lists without writing.
- [ ] Task mutations remain serialized and atomic.
- [ ] Frontend and core tests cover the interactive and markdown behaviors.

## Non-Functional Requirements

| Area | Requirement |
| --- | --- |
| Data integrity | Every task mutation acquires the repository lock before reading and writes atomically. |
| Accessibility | Task controls have clear labels and support keyboard interaction. |
| Compatibility | Existing task-log format and CLI task commands remain unchanged. |

## Child User Stories

| Story ID | Title | Complexity | Points |
| --- | --- | --- | --- |
| US-048 | Manage story tasks from the web UI | medium | 5 |

## Definition of Done (Epic Level)

- [ ] The child story satisfies every acceptance scenario.
- [ ] The full verification suite, `kanban validate .`, and `kanban doctor .` pass.

## Notes and Open Questions

| # | Question / Assumption | Owner | Resolved |
| --- | --- | --- | --- |
| 1 | Explicit up/down controls are preferred to adding a drag-and-drop dependency. | Tooling lead | Yes |

_Template version: 1.0 (2026-06-21)_
