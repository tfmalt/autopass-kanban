---
id: US-024
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 3
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Semantic exit codes derived from KanbanErrorCode

---

## Story Statement

**As a** script author automating `kanban`,
**I want** the process exit code to distinguish bad-input from not-found from
not-initialized,
**so that** my script can react without parsing JSON.

---

## Background

`crates/cli/src/main.rs:145-155` maps every error to `exit(1)`. `KanbanErrorCode`
already carries `InvalidArgument`/`StoryNotFound`/`NotInitialized`/`InvalidStatus`
in JSON mode, but that distinction is lost in the human-mode process exit.

**Complexity: low** — map `KanbanErrorCode` to sysexits-style codes at the exit
boundary.

---

## Acceptance Criteria

**Scenario 1: InvalidArgument exits 64**

```gherkin
Given a `kanban story move` with an invalid status
When it fails in human mode
Then the process exits with code 64
```

**Scenario 2: StoryNotFound exits 2**

```gherkin
Given a `kanban story show <unknown-id>`
When it fails
Then the process exits with code 2
```

**Scenario 3: NotInitialized exits 78**

```gherkin
Given a repo without `.kanban`
When any command requiring config runs
Then the process exits with code 78
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Observability** | Exit codes follow sysexits conventions where applicable              |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/main.rs`, `crates/cli/src/json_out.rs`
- **Suggested patterns:** Add `fn exit_code_for(code: KanbanErrorCode) -> i32` (e.g. `InvalidArgument`→64, `NotFound`→2, `NotInitialized`→78, others→1) and use it in both human and JSON exit paths.
- **Testing approach:** Integration tests asserting exit codes for each category.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Exit codes mapped from `KanbanErrorCode`
- [ ] Both human and JSON paths use the mapping
- [ ] Integration tests assert codes per category
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-025: Typed KanbanError enum          | Story   | Draft     | Provides the authoritative code source |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add `exit_code_for` mapping
- Use it in human and JSON exit paths
- Add exit-code integration tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Use sysexits.h codes or a custom scheme?                         | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
