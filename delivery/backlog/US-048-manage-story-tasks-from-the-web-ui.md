---
id: US-048
type: user-story
status: done
epic: EP-006
sprint: S001.rolling-thunder
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-08-25T09:19:43+0200
work_done: 2026-08-25T10:22:32+0200
created: 2026-08-25T09:18:12+0200
updated: 2026-08-25T10:22:32+0200
activated: 2026-08-25T09:19:42+0200
---

# User Story: Manage story tasks from the web UI

---

## Story Statement

**As a** developer,
**I want** to create, order, update, and delete tasks while viewing a story in
the web UI,
**so that** I can maintain a story's executable work plan without switching to
the CLI.

---

## Background

The story detail modal currently lets a developer change task status only.
Adding, editing, ordering, or removing tasks requires a CLI command, interrupting
planning while reviewing the story. The web server must delegate every mutation
to locked, atomic core operations over the canonical sibling `.tasks.md` file.

---

## Acceptance Criteria

**Scenario 1: Create a task**

```gherkin
Given a developer is viewing a story with no task file or existing tasks
When they enter a title and save a new task
Then the web UI creates the next task ID in the story's canonical sibling task file
And the new task is shown in the story detail view
```

**Scenario 2: Update a task**

```gherkin
Given a developer expands an existing task in the story detail view
When they change its title, description, tags, or status and save
Then only that task's values are updated in the canonical task log
```

**Scenario 3: Order and delete tasks safely**

```gherkin
Given a story has multiple tasks
When a developer moves a task up or down
Then the rendered order matches the canonical task-log order
When they delete a task and confirm the action
Then that task is removed from the task log
And incomplete, duplicate, or unknown reorder requests are rejected without a write
```

> Add more scenarios as needed. Include at least one error/edge case scenario.

---

## Non-Functional Requirements

> Specify any requirements that go beyond functional correctness.
> Inherit from parent Epic unless explicitly overridden here.

| Area | Requirement |
| --- | --- |
| **Data integrity** | Every mutation acquires the repository lock before reading and writes atomically. |
| **Accessibility** | Buttons and fields have explicit labels and work with a keyboard. |
| **Compatibility** | Existing task log format and CLI behavior remain unchanged. |

---

## Technical Notes

> Guidance for developers and AI assistants on expected implementation
> approach. This section is non-prescriptive — teams can deviate with
> justification. Include relevant architecture patterns, module hints,
> or integration points.

- **Requirement refs:** `EP-006#acceptance-criteria`.
- **Scenarios:** Create, update, order, and delete story tasks.
- **Component / Module:** `crates/core/src/story.rs`, `crates/web-server/src/handlers/mod.rs`, `web/src/components/StoryModal.tsx`.
- **Key integration points:** Story task routes, React Query invalidation, and sibling `.tasks.md` files.
- **Suggested patterns:** Explicit up/down controls; core validates the complete reorder list before an atomic write.
- **Testing approach:** Core mutation tests and StoryModal interaction tests.
- **Migration / backward compatibility:** Preserve task headings, IDs, and task-file format; no new statuses or persisted state.

### Estimation Rules

Frontmatter is the metadata source of truth. Do not duplicate frontmatter fields
in a `## Metadata` section inside the story body.

`story_points` is the only estimation field stored in frontmatter. During human
drafting it may temporarily use either a numeric Fibonacci value or a T-shirt
alias.

| T-shirt size | Story points |
| ------------ | ------------ |
| `XXS`        | `1`          |
| `XS`         | `2`          |
| `S`          | `3`          |
| `M`          | `5`          |
| `L`          | `8`          |
| `XL`         | `13`         |
| `XXL`        | `21`         |

> The authoritative alias and allowed-value lists live in the `story_points`
> block of `.kanban/settings.json`. If they differ from this table, that file
> wins — it is what `kanban validate` enforces.

- `story_points` is mandatory on all User Stories
- default `story_points` is `5` when no different estimate has yet been agreed
- draft aliases `XXS`, `XS`, `S`, `M`, `L`, `XL`, and `XXL` are allowed during manual authoring
- tools and AI agents should normalize draft aliases to numeric Fibonacci values on first write
- the canonical persisted value in the repository is numeric `story_points`, not the T-shirt label

### Workflow Lifecycle Fields

- `assignee` is a standard frontmatter field on all User Stories; use `Name <email>` when known
- `created`, `updated`, `activated`, `work_started`, and `work_done` use full local ISO 8601 timestamps with numeric timezone offset (for example `2026-05-28T14:05:54+0200`)
- `work_started` stays empty when a story is created
- set `work_started` the first time the story moves from `todo` to `in-progress`
- planning a story into a sprint normally moves it to `planned`; move it to `todo` when it is ready for execution
- preserve `work_started` if the story moves back, is blocked, or carries over to
  a new sprint
- set `work_done` when the story moves to `done`

---

## Definition of Done

> All items below must be met before this story can be accepted.
> This list reflects project team standards.

- [ ] All acceptance scenarios are covered by frontend and core tests.
- [ ] Core task mutations lock before reading and write atomically.
- [ ] Web API mutations serialize through `AppState::write_lock`.
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass.
- [ ] `kanban validate .` and `kanban doctor .` pass.
- [ ] Story is demo-ready for sprint review.

---

## Dependencies

| Dependency | Type | Status | Notes |
| --- | --- | --- | --- |
| `add_task_to_story`, `update_task_in_story`, `delete_task_from_story` | Core API | Available | Existing task mutations to extend and expose. |
| `AppState::write_lock` | Web server | Available | Serializes web mutations. |

---

## Actual Implementation

> Added during sprint S001.rolling-thunder. Reflects the implemented slice.

- Added locked, atomic core task reordering that accepts every current task ID
  exactly once; create, update, and delete now acquire the repository lock
  before reading their task file.
- Added web-server task create, reorder, and delete routes alongside the
  existing update route, all serialized through `AppState::write_lock`.
- Added story-modal forms for task creation and full-field editing, accessible
  up/down ordering buttons, and explicit delete confirmation.
- Verified with `npm test`, `npm run build`, `cargo fmt --all -- --check`,
  `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo build`, and `kanban validate .`.
- `kanban doctor .` still reports 35 pre-existing stories without sprint
  assignments; no finding concerns US-048 or S001.rolling-thunder.

---

## Sprint Task Log Guidance

> Sprint execution tasks are tracked in a sibling `.tasks.md` file when this
> story is activated into a sprint. Keep that file lightweight.

Expected task log structure:

- `# Tasks for <US-ID>` file heading with optional lightweight context lines
- task heading with a lightweight task ID and verb-first title
- `Status:` using canonical workflow keywords such as `todo`, `in-progress`, `blocked`, or `done`
- `Tags:` with short labels
- `Description:` with a short note about the concrete work being done
- no `---` separators; tasks are delimited by the next `## TASK-...` heading

Keep detailed requirements, acceptance criteria, testing expectations, and
implementation guidance in this User Story rather than duplicating them in a
separate task specification document.

---

## Notes and Open Questions

| # | Question / Assumption | Owner | Due | Resolved |
| --- | --- | --- | --- | --- |
| 1 | Explicit up/down buttons are sufficient for task ordering; drag and drop is out of scope. | Tooling lead | 2026-08-25 | Yes |
| 2 | Deletion requires explicit browser confirmation. | Tooling lead | 2026-08-25 | Yes |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
