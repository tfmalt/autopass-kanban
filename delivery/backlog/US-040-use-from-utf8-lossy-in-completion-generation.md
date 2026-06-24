---
id: US-040
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

# User Story: Use from_utf8_lossy in completion generation

---

## Story Statement

**As a** script author evaluating `kanban completion`,
**I want** generation to never panic on non-UTF8 output,
**so that** my shell does not receive a Rust backtrace instead of a completion script.

---

## Background

`crates/cli/src/main.rs:787` and `json_out.rs:71` do
`String::from_utf8(buf).expect("clap_complete output should be utf8")`, which
panics if the output is ever non-UTF8.

**Complexity: simple** — use `from_utf8_lossy`.

---

## Acceptance Criteria

**Scenario 1: Non-UTF8 output does not panic**

```gherkin
Given `clap_complete::generate` produces non-UTF8 bytes
When the CLI converts to a string
Then it uses `from_utf8_lossy` and returns a usable script (no panic)
```

**Scenario 2: Normal completion unchanged**

```gherkin
Given normal UTF8 output
When the CLI converts
Then the script is identical to before
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Robustness**   | No `.expect()` on `String::from_utf8` in completion paths            |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/main.rs:787`, `crates/cli/src/json_out.rs:71`
- **Suggested patterns:** Replace `.expect(...)` with `String::from_utf8_lossy(&buf).into_owned()`.

### Estimation Rules

`story_points` is `1` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Both `from_utf8(...).expect()` sites replaced with `from_utf8_lossy`
- [ ] Completion generation tests pass
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

- Replace `.expect()` with `from_utf8_lossy` at both sites
- Verify completion tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
