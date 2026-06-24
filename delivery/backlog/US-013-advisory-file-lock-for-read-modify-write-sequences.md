---
id: US-013
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 8
work_started: 2026-06-24T11:17:15+0200
work_done: 2026-06-24T11:17:21+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T11:17:21+0200
---

# User Story: Advisory file lock for read-modify-write sequences

---

## Story Statement

**As a** developer running the CLI and the web server concurrently,
**I want** read-modify-write sequences on backlog files to serialize via an
advisory lock,
**so that** two concurrent `story move` calls on the same story cannot silently
lose one update.

---

## Background

`read_repository` → mutate → `fs::write` is a classic TOCTOU. The web server
(`crates/web-server`) and CLI can run concurrently, and `regenerate_sprint_roster`
re-reads the whole repository on every mutation, compounding the race. There is
no file locking today.

**Complexity: high** — needs a locking strategy, a shared helper, threading
through all mutation entry points, a documented concurrency model, and a
concurrency test.

---

## Acceptance Criteria

**Scenario 1: Concurrent mutations serialize**

```gherkin
Given two `kanban story move` calls on the same story run simultaneously
When both complete
Then both updates are applied in sequence (no lost update)
And the final frontmatter reflects the last completed move
```

**Scenario 2: Lock is released on error**

```gherkin
Given a mutation fails mid-write
When the error is propagated
Then the advisory lock is released and the next command can proceed
```

**Scenario 3: Concurrency model is documented**

```gherkin
Given the repository after the change
When a developer reads AGENTS.md or the new ADR
Then the locking model (advisory, per-repo, blocking or fail-fast) is described
```

---

## Non-Functional Requirements

| Area                | Requirement                                                              |
| ------------------- | ------------------------------------------------------------------------ |
| **Data integrity**  | Concurrent writers never silently overwrite each other                   |
| **Portability**     | Lock works on Unix (fcntl/flock) and Windows (LockFileEx)                |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/repository.rs` (lock helper), all mutation entry points in `story.rs`/`sprint.rs`/`epic.rs`/`doctor.rs`, web-server mutation handlers
- **Suggested patterns:** `fs2::FileExt::lock_exclusive` on `.kanban/.lock` around any read-modify-write; for the web server, serialize all writes through a `tokio::sync::Mutex` in addition. Add `fs2` as a dependency or use `fs4`/`fd-lock`.
- **Testing approach:** Spawn two threads/tasks that mutate the same story and assert both updates land; assert a held lock blocks or errors a second writer.
- **Migration / backward compatibility:** Advisory locks are not enforced across processes that bypass the helper — document this.

### Estimation Rules

`story_points` is `8` (complexity: high).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Advisory lock helper exists in `core` and wraps every mutation entry point
- [ ] Web-server mutation handlers serialize via a mutex
- [ ] Concurrency test demonstrates no lost update
- [ ] ADR documents the locking model
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-012: Atomic writes                   | Story   | Draft     | Lock + atomic writes together guarantee integrity |
| `fs2` or equivalent locking crate       | Dependency | Available | Add to workspace deps                  |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Choose locking crate and add dependency
- Add lock helper around read-modify-write in `core`
- Add web-server write mutex
- Write concurrency test
- Write ADR for concurrency model

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Per-repo or per-sprint lock granularity?                         | Tooling lead | 2026-07-04 | Yes — per-repo. A single `.kanban/.lock` guards the whole backlog; matches the single-source-of-truth model and protects cross-sprint operations like roster regeneration. |
| 2   | Block or fail-fast when the lock is held?                        | Tooling lead | 2026-07-04 | Yes — block with a timeout (default 5 s), then fail with a clear "backlog is locked" error. The web server additionally serializes writes through a `tokio::sync::Mutex`. |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
