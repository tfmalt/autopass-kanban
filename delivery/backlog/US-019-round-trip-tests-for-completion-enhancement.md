---
id: US-019
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T16:28:51+0200
work_done: 2026-06-24T16:30:52+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:30:52+0200
---

# User Story: Round-trip tests for shell completion script enhancement

---

## Story Statement

**As a** maintainer changing clap definitions or help text,
**I want** tests that assert the dynamic completion helpers are actually wired
into the generated scripts,
**so that** a `clap_complete` version bump or help-text edit cannot silently
regress completions to `_default`.

---

## Background

`crates/cli/src/completion.rs:195-424` is ~230 lines of `str::replace` over
generated clap output; mismatches silently no-op. The existing
`tests/completion.rs` only asserts top-level command names appear, not that
`_kanban_sprint_names` is wired into the right argument.

**Complexity: medium** — add round-trip tests asserting replacements actually
mutated the script and dynamic helpers are attached to the right arguments.

---

## Acceptance Criteria

**Scenario 1: Sprint-name helper is wired into sprint-show**

```gherkin
Given `kanban completion zsh` output
When the test inspects the sprint-show argument
Then the helper `_kanban_sprint_names` is attached to it, not `_default`
```

**Scenario 2: Each inject_bash call actually mutated the script**

```gherkin
Given `kanban completion bash` output
When the test checks for a marker only present in the replacement
Then every `inject_bash_*` replacement is confirmed to have applied
```

**Scenario 3: Regression is caught**

```gherkin
Given a clap help text change that breaks a replacement
When the completion tests run
Then at least one test fails naming the un-applied replacement
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Maintainability** | Completion regressions are caught at test time, not at user runtime   |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/tests/completion.rs`, `crates/cli/src/completion.rs`
- **Suggested patterns:** Generate the script in-test, assert the dynamic helper name appears on the target argument line, and assert each `inject_bash_*` marker is present. Long-term, derive dynamic completions from clap metadata rather than patching generated text.
- **Testing approach:** Pure string-inspection tests on generated output.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Tests assert dynamic helpers are wired into the correct arguments
- [ ] Tests assert each `inject_bash_*` replacement actually applied
- [ ] A deliberate replacement-break is caught by a test
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-018: Status name consts              | Story   | Draft     | Reduces the surface that can drift      |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add round-trip completion generation tests
- Assert helper wiring and replacement markers
- Add a negative test for regression detection

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should the long-term fix (clap metadata-driven completion) be in scope? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
