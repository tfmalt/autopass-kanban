---
id: US-021
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 
work_done: 2026-06-24T17:29:21+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T17:29:21+0200
---

# User Story: Split the web-server god-module into focused modules

---

## Story Statement

**As a** maintainer of the web server,
**I want** the 2000-line `lib.rs` split into handler, DTO, metrics, team, and
sprint-IO modules,
**so that** concerns are isolated and the file is navigable.

---

## Background

`crates/web-server/src/lib.rs` (2022 lines) contains the router, 25+ DTO
structs, the metrics engine (`build_burndown`/`build_burnup`/`build_lead_time`/
`build_velocity`/`build_forecast`, lines 984-1339), team loading, sprint-file
mutation, and unit tests all in one file.

**Complexity: medium** — pure structural refactor; no behavior change.

---

## Acceptance Criteria

**Scenario 1: Handlers live in a handlers module**

```gherkin
Given the refactored crate
When inspecting `crates/web-server/src/`
Then handler functions are in a `handlers/` module tree, not `lib.rs`
```

**Scenario 2: DTOs are separated**

```gherkin
Given the refactored crate
When inspecting the DTO structs
Then they live in a `dto.rs` module
```

**Scenario 3: Metrics engine is its own module**

```gherkin
Given the refactored crate
When inspecting the burndown/burnup/lead-time/velocity/forecast functions
Then they live in a `metrics.rs` module
```

**Scenario 4: No behavior change**

```gherkin
Given the refactored crate
When the existing web-server tests run
Then they pass unchanged
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Maintainability** | No single source file exceeds ~500 lines after the split              |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` → `handlers/`, `dto.rs`, `metrics.rs`, `team.rs`, `sprint_io.rs`
- **Suggested patterns:** Move functions verbatim into new modules, re-export from `lib.rs` to preserve the public API. Keep markdown mutation helpers for US-022 to relocate.
- **Testing approach:** Existing tests must pass with no edits.
- **Migration / backward compatibility:** Public API of the crate is unchanged.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `lib.rs` split into `handlers/`, `dto.rs`, `metrics.rs`, `team.rs`, `sprint_io.rs`
- [ ] No source file exceeds ~500 lines
- [ ] Existing tests pass unchanged
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-022: Move markdown helpers to core   | Story   | Draft     | Do after the split for cleaner relocation |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Create module files and move functions
- Re-export from `lib.rs`
- Verify tests pass

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should handlers be one file or a directory of per-resource files? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
