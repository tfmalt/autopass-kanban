---
id: US-026
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T16:31:03+0200
work_done: 2026-06-24T16:34:39+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:34:39+0200
---

# User Story: Expand validate.rs to catch writer-produced corruption

---

## Story Statement

**As a** developer relying on `kanban validate`,
**I want** validation to catch the corruption the writer can produce
(unsafe `task_file`, duplicate story IDs, out-of-tree paths),
**so that** bad data is reported rather than silently deduped or written.

---

## Background

`validate.rs` has no rule for `task_file` shape (covered by US-008),
duplicate story IDs are silently deduped by `unique_story_overviews`, and
out-of-tree canonicalized paths are not checked. `validate_story` re-discovers
config per story via `load_kanban_config(parent)`, which can pick up the wrong
`.kanban` in submodules.

**Complexity: medium** — add rules and pass the already-loaded config in.

---

## Acceptance Criteria

**Scenario 1: Duplicate story IDs are reported**

```gherkin
Given two story files with the same `id`
When `kanban validate .` runs
Then it reports a `duplicate-story-id` issue naming both files
```

**Scenario 2: Out-of-tree path is reported**

```gherkin
Given a story whose canonicalized path is outside the backlog root
When `kanban validate .` runs
Then it reports an `out-of-tree-story-path` issue
```

**Scenario 3: validate uses the loaded config, not per-story rediscovery**

```gherkin
Given the refactored validate
When `validate_story` runs
Then it accepts the already-loaded `&KanbanConfig` and does not call `load_kanban_config(parent)`
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Correctness**  | Validation catches duplicate IDs, unsafe paths, and config drift      |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/validate.rs`
- **Suggested patterns:** Pass `&KanbanConfig` into `validate_story`; add `duplicate-story-id` (collect IDs across the tree) and `out-of-tree-story-path` rules. The `task_file` rule is added in US-008.
- **Testing approach:** Fixtures with duplicate IDs and an out-of-tree symlink; assert the rules fire.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `duplicate-story-id` and `out-of-tree-story-path` rules added
- [ ] `validate_story` accepts `&KanbanConfig`
- [ ] Tests cover duplicate IDs and out-of-tree paths
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-008: Path containment + task_file rule | Story | Draft     | Provides the `invalid-task-file-path` rule |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Pass loaded config into `validate_story`
- Add duplicate-ID detection
- Add out-of-tree path detection
- Add fixtures and tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should duplicate IDs be an error or a warning?                  | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
