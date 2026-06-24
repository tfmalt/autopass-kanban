# ADR-001: Advisory per-repo file locking model

- **Status:** Accepted
- **Date:** 2026-06-24
- **Supersedes:** none
- **Related:** US-013, EP-003

## Context

The kanban backlog is a markdown source of truth that can be mutated
concurrently by the `kanban` CLI and the local web server
(`kanban web start`). The read-modify-write pattern used by every mutation
(`read_repository` → mutate in memory → `fs::write`) is a classic TOCTOU: two
concurrent `story move` calls on the same story can silently lose one update,
and `regenerate_sprint_roster` re-reads the whole repository on every mutation,
compounding the race.

There was no file locking before US-013.

## Decision

We adopt an **advisory, per-repo, blocking-with-timeout** file lock:

- A single advisory lock file at `.kanban/.lock` guards the whole backlog.
  Per-repo granularity (rather than per-sprint) protects cross-sprint
  operations such as roster regeneration and keeps the trust model simple.
- The lock is acquired with an exclusive byte-range lock via the `fs4` crate
  (`FileExt::try_lock_exclusive`), polled at 50 ms intervals up to a
  **5 second** default timeout, after which the mutation fails fast with a
  clear "backlog is locked" error.
- The lock is held for the duration of the read-modify-write sequence by a
  `RepoLock` guard in `kanban-core`; dropping the guard releases the lock.
- Every public mutation entry point in `kanban-core`
  (`move_story_to_status_with_assignee`, `plan_story_into_sprint`,
  `delete_story`, `add_task_to_story`, `update_task_in_story`,
  `delete_task_from_story`, `update_story_frontmatter`,
  `update_epic_frontmatter`, `apply_doctor_fix`) acquires the guard.
- The web server adds a second, in-process `tokio::sync::Mutex<()>`
  (`AppState::write_lock`) that serializes all mutation handlers, so
  concurrent UI actions order cleanly and do not contend on the file lock
  within a single server process.

## Consequences

- **Positive:** Concurrent CLI and web-server mutations serialize; no silent
  lost updates. The lock is portable (fcntl on Unix, LockFileEx on Windows).
- **Negative:** Advisory locks are **not** enforced across processes that bypass
  the `RepoLock` helper. A process that writes backlog markdown directly
  (e.g. an editor, git, or a script) is not stopped. This is documented in
  `AGENTS.md`.
- **Risk:** A crashed process may leave the lock file on disk, but the byte-range
  lock is released by the OS when the file descriptor closes, so a leftover
  file does not block future acquires.
- **Future:** Atomic writes (US-012, temp-file + rename + fsync) combined with
  this lock give both crash-safety and concurrency safety.
