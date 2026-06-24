---
id: US-037
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 2
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Warn on corrupt settings.json in theme resolution

---

## Story Statement

**As a** user with a corrupted `.kanban/settings.json`,
**I want** `kanban` to print a one-line warning when it falls back to the
default color mode,
**so that** I am not silently given degraded output with no clue why.

---

## Background

`crates/cli/src/cli.rs:983-992` and `main.rs:174-177` do
`load_kanban_config(...).ok().map(...).unwrap_or(ColorMode::Auto)`, silently
degrading on a malformed config.

**Complexity: simple** — print a `theme.warning_label()` line on load failure.

---

## Acceptance Criteria

**Scenario 1: Corrupt settings warns**

```gherkin
Given a malformed `.kanban/settings.json`
When a non-JSON command resolves the theme
Then it prints a one-line warning naming the config file
And proceeds with the default color mode
```

**Scenario 2: JSON mode is unaffected**

```gherkin
Given `--format json` with a corrupt config
When the command runs
Then no warning is printed to stdout (it would break JSON)
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Diagnosability** | Silent degradation is replaced with a visible warning               |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/cli.rs`, `crates/cli/src/main.rs`
- **Suggested patterns:** Replace `.ok()` with a match that `eprintln!`s a `theme.warning_label()` line on `Err`, then falls back.

### Estimation Rules

`story_points` is `2` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Warning printed on corrupt config in non-JSON mode
- [ ] JSON mode unaffected
- [ ] Test covers the warning
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

- Replace silent `.ok()` with warn-and-fallback
- Add warning test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
