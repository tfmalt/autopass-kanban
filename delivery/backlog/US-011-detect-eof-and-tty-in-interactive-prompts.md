---
id: US-011
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T12:15:05+0200
work_done: 2026-06-24T12:19:10+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T12:19:10+0200
---

# User Story: Detect EOF and TTY in interactive prompts

---

## Story Statement

**As a** user scripting `kanban` (or piping `/dev/null`),
**I want** interactive prompts to detect a closed stdin and a non-TTY and bail
clearly,
**so that** `kanban doctor fix < /dev/null` cannot auto-apply every fix without
consent and `kanban sprint create < /dev/null` cannot write an empty headline
to the backlog.

---

## Background

`crates/cli/src/prompt.rs:10-18` treats `read_line` returning `Ok(0)` (EOF) as
empty input. `prompt_with_default` returns the default on EOF, and `prompt`
returns `""`. In `doctor_cli.rs:143-147` empty input is treated as `"y"`, so
`doctor fix < /dev/null` applies every fix. The `Choice` branch
(`doctor_cli.rs:178-194`) busy-loops on EOF because `read_line` keeps returning
`Ok(0)` instantly. `sprint create` passes the empty headline straight into
`create_sprint` with no non-empty validation.

**Complexity: medium** — TTY detection plus EOF handling across prompt helpers
and the two affected commands, with tests.

---

## Acceptance Criteria

**Scenario 1: Doctor fix on closed stdin does not auto-apply**

```gherkin
Given stdin is closed (`< /dev/null`)
When the user runs `kanban doctor fix`
Then it exits non-zero with a message like "standard input is closed; cannot prompt for fix confirmation"
And no fix is applied
```

**Scenario 2: Sprint create on closed stdin does not produce empty headline**

```gherkin
Given stdin is closed
When the user runs `kanban sprint create`
Then it exits non-zero before writing any sprint file
And no sprint file with an empty headline is created
```

**Scenario 3: Choice prompt on EOF exits instead of busy-looping**

```gherkin
Given a `Choice` prompt is displayed and stdin is closed
When `read_line` returns `Ok(0)`
Then the prompt returns an error instead of looping
```

**Scenario 4: Non-interactive flag bypasses prompts**

```gherkin
Given `--non-interactive` is supplied to `doctor fix`
When fixes are available
Then all safe automatic fixes are applied without prompting
And guided/manual fixes are skipped with a summary
```

---

## Non-Functional Requirements

| Area             | Requirement                                                            |
| ---------------- | ---------------------------------------------------------------------- |
| **Data integrity** | No backlog file is written as a side effect of an unanswerable prompt |
| **UX**            | EOF and non-TTY produce a single actionable error line                 |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/prompt.rs`, `crates/cli/src/doctor_cli.rs`, `crates/cli/src/main.rs` (sprint create path)
- **Suggested patterns:** Use `std::io::IsTerminal` (stable since 1.70) to detect TTY; on `read_line` returning `Ok(0)`, return `bail!("standard input is closed; ...")`. Add a non-empty check for `sprint create` headline. Add a `--non-interactive` flag to `doctor fix`.
- **Testing approach:** Unit tests feed an empty `Cursor` as stdin and assert the prompt errors; integration test runs `kanban doctor fix < /dev/null` and asserts non-zero exit and no file mutation.
- **Migration / backward compatibility:** Existing interactive use is unchanged; only the EOF/non-TTY path newly errors.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `prompt`/`prompt_with_default` detect EOF and return an error
- [ ] `Choice` prompt no longer busy-loops on EOF
- [ ] `sprint create` rejects empty headlines
- [ ] `doctor fix` gains `--non-interactive` and refuses to prompt on closed stdin
- [ ] Tests cover EOF for both commands
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| None                                    | -       | -         | Standalone safety fix                  |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add `IsTerminal` + EOF detection to `prompt.rs`
- Fix `Choice` busy-loop on EOF
- Add non-empty headline check to `sprint create`
- Add `--non-interactive` to `doctor fix`
- Add EOF integration tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should `--non-interactive` apply globally or per-subcommand?     | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
