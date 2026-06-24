---
id: US-030
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

# User Story: Stop swallowing missing .kanban in test cfg

---

## Story Statement

**As a** maintainer,
**I want** `load_kanban_config` to behave identically in test and production,
**so that** the "not initialized" error path has real test coverage.

---

## Background

`crates/core/src/config.rs:513-531` returns a default config under
`#[cfg(test)]` when `.kanban` is missing, so tests cannot exercise the
not-initialized error path. Several validate/doctor tests rely on the silent
default, masking bugs.

**Complexity: low** — use an explicit test helper and have `load_kanban_config`
always bail.

---

## Acceptance Criteria

**Scenario 1: load_kanban_config bails in test too**

```gherkin
Given a test directory without `.kanban`
When `load_kanban_config` is called
Then it returns the not-initialized error
```

**Scenario 2: Tests use an explicit default helper**

```gherkin
Given tests that need a default config
When they set up their fixture
Then they call `load_kanban_config_or_default` (or `testutil::init_config`) explicitly
```

**Scenario 3: Not-initialized path is tested**

```gherkin
Given the refactored code
When the not-initialized error path is exercised
Then at least one test asserts the error
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Test fidelity**   | Test and production behavior match for config loading                 |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/config.rs`, `crates/core/src/testutil.rs`
- **Suggested patterns:** Remove the `#[cfg(test)]` branch; provide `pub(crate) fn load_kanban_config_or_default(repo_root) -> KanbanConfig` for tests; `testutil::init_config` already exists.
- **Testing approach:** Add a test asserting `load_kanban_config` bails on missing `.kanban`.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `#[cfg(test)]` default branch removed
- [ ] Tests use explicit default helper
- [ ] Not-initialized error path covered by a test
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

- Remove `#[cfg(test)]` branch from `load_kanban_config`
- Add explicit test default helper
- Add not-initialized test
- Fix any tests relying on the silent default

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
