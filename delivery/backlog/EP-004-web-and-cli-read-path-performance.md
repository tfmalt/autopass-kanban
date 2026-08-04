---
id: EP-004
type: epic
status: done
phase: 1
owner: Thomas Malt / Tooling Lead
milestone: MP2
priority: 70
planned_start: 2026-08-03
planned_end: 2026-08-04
work_started: 2026-08-04T09:58:26+0200
work_done: 2026-08-04T09:58:26+0200
created: 2026-08-04T09:58:26+0200
updated: 2026-08-04T09:58:26+0200
---

# Epic: Web and CLI read-path performance

---

## Business Context

Profiling the embedded web board against a ~214-story backlog measured a 22.9 s
cold board LCP and a 24.5 s cold dashboard LCP. The frontend bundles were not
the cause. Serving one `/api/repository` request spawned roughly **6,300
`git rev-parse --show-toplevel` subprocesses**, because `load_kanban_config` was
called once per story, once per collector, and once per epic inside an N+1 loop:
`load_epics` invoked `find_epic` — itself a full repository read plus a full
epic-file rescan — once for every epic file.

The same defect made `kanban validate .` take 0.56 s and `kanban doctor .` take
0.87 s on a 41-story repository, with most of the wall time in subprocess spawns
rather than parsing. Low user time against high wall time is the signature.

The architectural rationale, the full cost model, the budgets, and the
measurement gate are recorded in `IMPROVEMENT_PLAN.md` at the repository root.
This Epic is the backlog representation of that plan.

---

## Business Value

- **Primary benefit:** The board and dashboard become usable. Cold LCP drops
  from ~23 s to under 300 ms, and `kanban validate`/`doctor` from ~0.6-0.9 s to
  ~0.02 s, on the same inputs.
- **Secondary benefit:** Losing the live-reload stream can no longer leave a
  client silently showing stale data — the defect that makes a "current state"
  tool untrustworthy.
- **Risk if not done:** The web UI is unusable on any backlog of realistic size,
  and the cost grows quadratically with the number of epics, so it gets worse
  with every epic added.

---

## Users and Stakeholders

| Role                            | Involvement                                                        |
| ------------------------------- | ------------------------------------------------------------------ |
| Developer using `kanban web`    | Waits on every board and dashboard load                            |
| Developer running `validate`/`doctor` | Runs these on every change; they gate the verification suite |
| AI agent maintaining a backlog  | Calls `--format json` read commands in a loop                      |
| Tooling lead                    | Owns the read-path invariants and the regression guards            |

---

## Scope

### In Scope

- Resolving the repository root and parsing `.kanban/settings.json` exactly once
  per repository read, by threading `&KanbanConfig` through config-aware
  collector and reader variants in `crates/core`.
- Removing the `find_epic` call from the web read model's epic projection.
- Deriving the repository snapshot, epic index, sprint buckets, metrics, and
  report from a single parsed `Repository`.
- A generated backlog fixture and read-path instrumentation counters, so the
  budgets are deterministic assertions rather than wall-clock timings.
- Coalescing filesystem-change bursts into one identified SSE event, with
  resumable event ids and explicit resynchronization on reconnect gaps and
  lagged subscribers.
- Bounded client-side query staleness with an SSE fallback, scoped invalidation,
  deferred modal chunk, and fixed-dimension loading skeletons.
- Static asset cache headers, `ETag`/`304`, range support, and build-time
  precompression (`US-034`).

### Out of Scope

- Any database, generated on-disk index, or persisted cache. Markdown remains
  the only source of truth.
- Incremental single-file reparse.
- Replacing React Query, React Router, or Recharts.
- Server-side rendering, service workers, or a visual redesign.
- Page-specific API contracts and a prewarmed read-model cache. These were
  specified as conditional on measurement gate G1, which passed, so they were
  deliberately not built. See `IMPROVEMENT_PLAN.md` §9.1 and §14.

---

## Acceptance Criteria

- [x] One `read_repository` performs exactly one `git rev-parse --show-toplevel`
      spawn and exactly one `.kanban/settings.json` parse, asserted by an
      injectable counter rather than by timing.
- [x] One web read-model build performs exactly one root resolution, one
      settings parse, one complete parse per story, and one per epic file.
- [x] No configuration load occurs inside any per-file loop in `crates/core`.
- [x] `load_epics` no longer calls `find_epic`; the epic projection is
      byte-equivalent to the `find_epic`-based algorithm on both fixture
      configurations.
- [x] `/api/metrics`, `/api/report`, and `/api/epics/<built-in function id>` each read the source
      once; the wire format is unchanged.
- [x] A burst of filesystem events produces exactly one SSE `change` event, and
      every event carries a monotonic generation as its event id.
- [x] A client that reconnects behind the current generation, or that falls
      behind the broadcast buffer, is told to resynchronize instead of silently
      losing changes.
- [x] Losing the SSE stream surfaces a non-blocking indicator and engages a
      polling fallback; it never leaves the client permanently stale.
- [x] Static assets return cache, validation, range, and encoding headers, and a
      repeat load of an unchanged hashed asset transfers zero bytes.
- [x] `cargo fmt --all -- --check`, `cargo test`,
      `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build`
      pass.
- [x] `kanban validate .` and `kanban doctor .` pass.

---

## Non-Functional Requirements

| Area                       | Requirement                                                                          |
| -------------------------- | ------------------------------------------------------------------------------------ |
| **Performance**            | Read-model build p95 <= 250 ms and `kanban doctor` <= 500 ms on a 250-story fixture   |
| **Performance**            | Board and dashboard cold LCP <= 1000 ms against a warm server and a cold browser cache |
| **Correctness**            | Repository, progress, metrics, and report served together derive from one source read |
| **Correctness**            | A client is never left silently stale after a mutation, external edit, or `git pull`  |
| **Data integrity**         | `RepoLock`, `AppState::write_lock`, and `ensure_path_inside` guarantees are unchanged  |
| **Backward compatibility** | No public `crates/core` API removed; the HTTP wire format is unchanged                 |

---

## Architecture Considerations

- **Relevant architecture principles:** Markdown stays the only persisted source
  of truth. No cache is introduced that is not fully derivable from it.
- **Key patterns in play:** explicit `&KanbanConfig` threading instead of
  memoizing filesystem state inside a library crate; one pure read-model builder
  feeding every projection; debounce-with-ceiling coalescing; monotonic
  generations as resumable SSE event ids; build-time asset fingerprinting.
- **Rejected alternative:** memoizing `resolve_repo_root`/`load_kanban_config`
  in a process-local map keyed by canonical path. Roughly thirty lines instead
  of three hundred, but a library crate that silently caches filesystem state is
  a latent bug for the CLI's `config set` path and for any test that mutates
  `settings.json` within one process. Explicit data flow is what `AGENTS.md`
  requires.
- **Known risks or constraints:** the read-path counters are thread-local and
  compiled only under `cfg(test)` or the `test-support` feature, so they add no
  global mutable state to a release build.

---

## Dependencies

| Dependency | Type  | Status      | Notes                                                        |
| ---------- | ----- | ----------- | ------------------------------------------------------------ |
| EP-003     | Epic  | In Progress | `US-034` (static assets) belongs to EP-003 and is done here   |
| US-023     | Story | Done        | Blocking I/O must stay inside `spawn_blocking`                |
| US-013     | Story | Done        | Advisory locking semantics must be preserved unchanged        |

---

## Child User Stories

| Story ID | Title                                                                    | Complexity | Points |
| -------- | ------------------------------------------------------------------------ | ---------- | ------ |
| US-042   | Reproducible performance harness and backlog fixture generator            | medium     | 5      |
| US-043   | Resolve kanban configuration once per repository read                     | high       | 8      |
| US-044   | Build the web read model in a single repository pass                      | high       | 8      |
| US-045   | Coalesce filesystem events and give SSE resumable identity                | medium     | 5      |
| US-046   | Bounded query staleness, SSE fallback, and deferred modal chunk           | medium     | 5      |

`US-034` (static asset caching and range headers) was completed as part of this
work but stays under `EP-003`, which already owned it.

---

## Definition of Done (Epic Level)

- [x] Every child story is `done`.
- [x] Measurement gate G1 is recorded in `IMPROVEMENT_PLAN.md` §9.1 with the
      before/after numbers and the Phase 3 decision.
- [x] The full verification suite passes.
- [x] The workspace version is bumped per `AGENTS.md`.

---

## Notes and Open Questions

| #   | Question / Assumption                                                          | Owner        | Due        | Resolved |
| --- | ------------------------------------------------------------------------------ | ------------ | ---------- | -------- |
| 1   | Should the conditional Phase 3 work be built? Gate G1 says no; recorded in §9.1 | Tooling lead | 2026-08-04 | Yes      |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic Epic template derived from the kanban tooling conventions_
