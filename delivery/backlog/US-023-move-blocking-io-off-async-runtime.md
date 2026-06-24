---
id: US-023
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 
work_done: 2026-06-24T17:36:11+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T17:36:11+0200
---

# User Story: Move blocking I/O off the web-server async runtime

---

## Story Statement

**As a** developer using the web UI with a large or slow-to-read backlog,
**I want** synchronous filesystem and subprocess calls moved off the async
worker pool,
**so that** a slow disk or `git` invocation cannot stall SSE delivery and other
requests.

---

## Background

`crates/web-server/src/lib.rs` runs `std::fs` reads/writes and
`Command::new("git")` directly inside async handlers on the multi-threaded
Tokio runtime (lines 466, 558, 729, 886, 1424, 1569). `git_branch` spawns a
subprocess on every `/api/config` call.

**Complexity: medium** — wrap blocking work in `spawn_blocking` or switch to
`tokio::fs`/`tokio::process`; cache `git_branch`.

---

## Acceptance Criteria

**Scenario 1: Blocking fs calls do not run on the async worker**

```gherkin
Given a web-server handler that reads the repository
When it performs filesystem I/O
Then the I/O runs inside `spawn_blocking` (or uses `tokio::fs`)
```

**Scenario 2: git branch is cached**

```gherkin
Given repeated calls to `/api/config`
When the handler resolves the git branch
Then `Command::new("git")` is invoked at most once per watcher event, not per request
```

**Scenario 3: SSE stays responsive under load**

```gherkin
Given a slow filesystem and an active SSE subscriber
When mutation requests are processed
Then SSE keep-alive comments continue to be sent
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Performance**  | Async worker pool is not blocked by synchronous I/O                   |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` (handlers, `load_repository_snapshot`, `load_sprints`, `load_team`, `git_branch`)
- **Suggested patterns:** Wrap whole snapshot loads in a single `spawn_blocking`; switch `git_branch` to `tokio::process::Command` and cache the result in the app state invalidated by the file watcher.
- **Testing approach:** Existing handler tests; add a test asserting `git_branch` is cached.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Blocking I/O wrapped in `spawn_blocking` or moved to `tokio::fs`/`tokio::process`
- [ ] `git_branch` cached
- [ ] Handler tests pass
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-021: Split web-server module         | Story   | Draft     | Easier to apply per-handler after split |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Wrap snapshot load in `spawn_blocking`
- Move per-handler blocking reads to `tokio::fs`
- Cache `git_branch` in app state
- Add caching test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | `spawn_blocking` per-request vs a dedicated blocking pool?      | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
