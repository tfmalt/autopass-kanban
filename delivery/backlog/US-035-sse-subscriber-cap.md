---
id: US-035
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 2
work_started: 
work_done: 2026-06-25T09:13:56+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-25T09:13:56+0200
---

# User Story: SSE subscriber cap

---

## Story Statement

**As a** developer running the web server,
**I want** the number of concurrent SSE subscribers bounded,
**so that** a runaway client cannot exhaust file descriptors and memory.

---

## Background

`crates/web-server/src/lib.rs:330,647-661` uses `broadcast::channel(128)` but
does not limit the number of subscribers.

**Complexity: simple** — track an `AtomicUsize` and reject beyond a cap.

---

## Acceptance Criteria

**Scenario 1: Subscribers beyond cap are rejected**

```gherkin
Given the SSE subscriber cap is 64 and 64 clients are connected
When a 65th client connects to `/api/events`
Then it receives 503 and no new subscription is created
```

**Scenario 2: Disconnect frees a slot**

```gherkin
Given a subscriber disconnects
When a new client connects
Then it is accepted
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Reliability**  | SSE subscriber count is bounded                                      |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` (`api_events`, app state)
- **Suggested patterns:** `Arc<AtomicUsize>` incremented on connect, decremented on drop via a guard; reject with 503 over the cap.

### Estimation Rules

`story_points` is `2` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Subscriber count tracked and capped
- [ ] Over-cap returns 503; disconnect frees a slot
- [ ] Test verifies the cap
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

- Add `AtomicUsize` subscriber counter to app state
- Increment/decrement with a guard
- Return 503 over cap; add test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Default cap value? 64?                                          | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
