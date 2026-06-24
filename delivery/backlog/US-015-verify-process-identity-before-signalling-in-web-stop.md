---
id: US-015
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T16:20:33+0200
work_done: 2026-06-24T16:22:30+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:22:30+0200
---

# User Story: Verify process identity before signalling in web stop

---

## Story Statement

**As a** developer running `kanban web stop` after a reboot,
**I want** the command to verify the recorded PID actually belongs to a
`kanban web serve` process before signalling it,
**so that** a recycled PID cannot cause `web stop` to SIGTERM/SIGKILL an
unrelated process.

---

## Background

`crates/cli/src/web.rs:389-409,732-766` parses the PID file and trusts
`process_exists(pid)`. After a reboot the recorded PID may belong to an
unrelated process; `stop_web` sends SIGTERM (escalating to SIGKILL) to whatever
owns it, with no command-name verification.

**Complexity: medium** — needs a startup cookie or command-name check plus
cross-platform process inspection.

---

## Acceptance Criteria

**Scenario 1: Stop targets only a real kanban web process**

```gherkin
Given the PID file points at a running `kanban web serve` process
When `kanban web stop` runs
Then it verifies the process command and sends SIGTERM
```

**Scenario 2: Recycled PID is not signalled**

```gherkin
Given the PID file points at a PID now owned by an unrelated process
When `kanban web stop` runs
Then it detects the mismatch, reports a stale PID file, and removes it without signalling
```

**Scenario 3: Dead PID is cleaned up**

```gherkin
Given the PID file points at a PID that no longer exists
When `kanban web stop` runs
Then it removes the stale PID file and reports the server is not running
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Safety**       | No signal is sent to a process whose command is not `kanban web serve` |
| **Portability**  | Process command check works on macOS, Linux, and Windows             |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/web.rs`
- **Suggested patterns:** Either (a) record a startup cookie in the PID file and have the web server echo it via a localhost status endpoint, or (b) read the process command name (`ps -o comm= -p {pid}` on Unix, `OpenProcess` + module query on Windows) and require it to match `kanban`.
- **Testing approach:** Test with a PID pointing at the current test process (mismatch) and at a spawned `kanban web serve` (match).
- **Migration / backward compatibility:** Old PID files without the cookie/command field are treated as unverifiable and cleaned up, not signalled.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `web stop` verifies process identity before signalling
- [ ] Recycled-PID and dead-PID cases are handled safely
- [ ] Tests cover mismatch, match, and missing-PID scenarios
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

- Add startup cookie or command-name verification
- Update `read_web_process_state` and `stop_web`
- Add mismatch/match/missing-PID tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Cookie via HTTP endpoint vs process command inspection?          | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
