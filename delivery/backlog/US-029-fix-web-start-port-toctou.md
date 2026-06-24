---
id: US-029
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

# User Story: Fix web start port TOCTOU

---

## Story Statement

**As a** developer running `kanban web start`,
**I want** the web child to bind without a port-availability race,
**so that** another process cannot grab the port between the probe and the
child bind, leaving a stale PID file.

---

## Background

`crates/cli/src/web.rs:368-387` binds a probe `TcpListener`, drops it, then
launches the web child on the same port. Another process can grab the gap; the
PID file is then written for a child that immediately exited.

**Complexity: low** — pass port `0` and use the port file as source of truth,
or hold the listener and pass the FD.

---

## Acceptance Criteria

**Scenario 1: No port race**

```gherkin
Given `kanban web start` launches the child
When another process is contending for the same port
Then the child binds its own port (via `0`) and reports it via the port file
And the CLI reads the actual bound port from the port file
```

**Scenario 2: Stale PID file is not written for a dead child**

```gherkin
Given the child fails to bind
When the CLI detects the failure
Then it does not write a PID file pointing at a dead process
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Reliability**  | No TOCTOU window between probe and bind                              |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/web.rs`, `crates/web-server/src/lib.rs`
- **Suggested patterns:** Pass `KANBAN_WEB_PORT=0` to the child; the web server writes the actual bound port to the port file; the CLI reads it. Alternatively pass the probe listener FD via `SC_PASSFD`/`WSAPROTOCOL_INFO`.
- **Testing approach:** Test that the CLI reads the port from the port file rather than assuming the requested port.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Port race eliminated (port `0` + port file, or FD passing)
- [ ] No stale PID file for a dead child
- [ ] Test verifies actual-bound-port reporting
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

- Pass port `0` to child and read port file
- Or implement FD passing
- Add port-reporting test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Port `0` + port file vs FD passing — which is simpler cross-platform? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
