---
id: US-039
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 1
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Delete dead inject_bash_story_update completion code

---

## Story Statement

**As a** maintainer,
**I want** the dead `inject_bash_story_update` function removed,
**so that** ~170 lines of stale raw strings do not obscure the active
`inject_bash_story_update_dynamic`.

---

## Background

`crates/cli/src/completion.rs:1224-1394` is `#[allow(dead_code)]` and superseded
by `inject_bash_story_update_dynamic` at line 1396.

**Complexity: simple** — delete the dead function.

---

## Acceptance Criteria

**Scenario 1: Dead function is gone**

```gherkin
Given the refactored `completion.rs`
When grepping for `fn inject_bash_story_update`
Then only the `_dynamic` variant remains
```

**Scenario 2: Build and tests still pass**

```gherkin
Given the deletion
When `cargo test` runs
Then all completion tests pass
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Maintainability** | No dead completion helpers retained                                   |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/completion.rs:1224-1394`
- **Suggested patterns:** Delete the function and its `#[allow(dead_code)]`.

### Estimation Rules

`story_points` is `1` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `inject_bash_story_update` deleted
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| None                                    | -       | -         | Standalone                             |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Delete the dead function and its allow attribute
- Verify completion tests pass

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
