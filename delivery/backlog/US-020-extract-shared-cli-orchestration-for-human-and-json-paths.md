---
id: US-020
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T16:38:17+0200
work_done: 2026-06-24T16:47:48+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:47:48+0200
---

# User Story: Extract shared CLI orchestration for human and JSON output paths

---

## Story Statement

**As a** maintainer changing story-list scope or sprint-create input handling,
**I want** the orchestration logic to exist once and be called by both the human
and JSON output paths,
**so that** the two paths cannot drift in behavior or error messaging.

---

## Background

`main.rs:476-506` and `json_out.rs:396-435` re-implement story-list scope
resolution (`all`/`next`/`sprint`/`current`/feature-disabled-fallback) with
divergent error paths. `main.rs:307-345` and `json_out.rs:925-950` duplicate the
sprint-create `CreateSprintInput` builder. Human mode `bail!`s free-form while
JSON emits `KanbanErrorCode` variants.

**Complexity: medium** — extract two shared builders and call from both paths.

---

## Acceptance Criteria

**Scenario 1: Story-list scope resolved by one function**

```gherkin
Given both `kanban story list` (human) and `--format json`
When they resolve the list scope
Then both call the same `resolve_story_list_scope` function
And the error cases produce consistent outcomes
```

**Scenario 2: Sprint-create input built by one function**

```gherkin
Given both output paths build a `CreateSprintInput`
When they parse `--start`/`--end` and defaults
Then both call the same `build_create_sprint_input` function
```

**Scenario 3: No behavior regression**

```gherkin
Given the refactored CLI
When the existing json_output and completion integration tests run
Then they pass unchanged
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Maintainability** | Orchestration logic for shared commands exists exactly once           |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/{main.rs,json_out.rs}`, new shared helper module (e.g. `crates/cli/src/ops.rs` or in `core`)
- **Suggested patterns:** `fn resolve_story_list_scope(repo_root, args) -> Result<(label, Vec<StoryOverview>)>` and `fn build_create_sprint_input(repo_root, flags) -> Result<CreateSprintInput>`; call from both `run()` and `emit_json()`.
- **Testing approach:** Existing `tests/json_output.rs` covers both paths; add a unit test for the shared builders.
- **Migration / backward compatibility:** No user-facing behavior change.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `resolve_story_list_scope` and `build_create_sprint_input` extracted
- [ ] Both `main.rs` and `json_out.rs` call the shared functions
- [ ] Existing integration tests pass
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-025: Typed KanbanError enum          | Story   | Draft     | Pairs well; shared builders can return typed errors |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Extract `resolve_story_list_scope`
- Extract `build_create_sprint_input`
- Rewire both output paths
- Add shared-builder unit tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should the shared builders live in `core` or a `cli::ops` module? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
