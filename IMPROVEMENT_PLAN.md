# Board and Dashboard Loading Improvement Plan

Status: **Implemented.** Phase 1 and Phase 2 landed; gate G1 passed, so Phase 3
was deliberately not built. See §9.1 for the measured result and §19 for the
implementation record.
Prepared: 2026-08-03
Implemented: 2026-08-04 (workspace version 26.8.401)
Revision: 2 (scope reduced, measurement gate added, SSE reliability defect added)
Scope: Cold and first-use loading performance for the embedded web board and
dashboard, plus the identical defect in the `validate` and `doctor` CLI paths.

Profiling context: a local checkout of the AutoPASS IP 2.0 repository. That path
is measurement context only. Implementation and tests must use generated
fixtures and must not depend on that external checkout.

---

## 1. Executive Summary

The frontend bundles are not the bottleneck. The web server spawns roughly
**6,300 `git rev-parse` subprocesses to serve one `/api/repository` request**,
because `load_kanban_config` is called once per story, once per collector, and
once per epic inside an N+1 loop. At a measured 4.06 ms per subprocess spawn
this alone accounts for ~25 s of the observed ~19-23 s response time.

Removing that blowup is a two-to-three order of magnitude improvement and is
achieved by two changes totalling a few hundred lines: **load configuration once
per repository read**, and **stop calling `find_epic` inside `load_epics`**.

Revision 2 therefore restructures the work:

- **Phase 1 is mandatory** and captures essentially the entire win.
- **Phase 2 is independent** and fixes real defects (static asset caching, an
  SSE reliability hole, per-card query observers) regardless of Phase 1 results.
- **Phase 3 (page-specific endpoints and a prewarmed cache) is conditional**
  on a measurement gate. It is the most complex and highest-coordination work in
  the plan and must not be built until Phase 1 has been measured and shown
  insufficient.

Markdown remains the only persisted source of truth. Any cache introduced is
process-local, disposable, and fully derivable from the markdown repository.

### 1.1 What changed from revision 1

| Change | Reason |
|---|---|
| Nine work packages reduced to four mandatory + three conditional | Revision 1's own arithmetic showed Phase 1 alone meets the uncached budget |
| Measurement gate G1 inserted after Phase 1 | Revision 1 sequenced the cache unconditionally with no stop condition |
| `staleTime: Infinity` replaced with bounded staleness | SSE has no replay, no lag handling, and no client error handler; see §3.8 |
| Watcher debouncing promoted to Phase 1 | It is a standalone defect, not a cache implementation detail |
| Build-time asset precompression preferred over runtime middleware | `tower-http` is not a workspace dependency; embedded assets compress once |
| CLI `validate`/`doctor` added to scope | Same defect, same fix, already measurable in this repository |
| Backlog mapping added (§12) | Repository convention is that work lives in `delivery/backlog`, not root markdown |
| Risk register added (§13) | Revision 1 had rollbacks for the server but none for the frontend |

---

## 2. Measured Baseline

### 2.1 Environment

- Server: installed `kanban 26.7.304`
- Command: `kanban web serve --repo-root <ip-2.0 checkout>`
- Browser: local Chrome DevTools, no CPU or network throttling
- Repository under profiling: ~214 parsed stories, 27 epics, 173 tasks, five
  sprint snapshots; ~2.3 MB of backlog markdown
- Each browser figure below is a single clean-navigation trace. Treat them as
  order-of-magnitude evidence, not statistics. WP-01 replaces them with
  n>=20 median/p95 figures.

### 2.2 Cold load

| Measurement | Observed |
|---|---:|
| Board cold LCP | 22.9 s |
| Dashboard cold LCP | 24.5 s |
| HTML TTFB | ~1 ms |
| `/api/repository`, isolated | 18.4-19.5 s |
| `/api/repository`, concurrent with metrics | 21.8 s |
| `/api/metrics`, concurrent with repository | 23.2 s |
| `/api/repository` response body | 686,707 bytes |
| `/api/metrics` response body | 8,869 bytes |
| Entry JS chunk | ~265 KB raw / ~85 KB gzip |
| Dashboard route chunk | ~386 KB raw / ~115 KB gzip |

The static shell paints in milliseconds and then shows `Loading...`. Nearly all
LCP time is render delay waiting on API data.

### 2.3 The same defect in the CLI

Measured in **this** repository (41 stories, release build):

```
kanban validate .   0.56 s wall  (0.14 s user)
kanban doctor .     0.87 s wall  (0.49 s user)
git -C . rev-parse --show-toplevel   4.06 ms/spawn (100-iteration mean)
```

Low user time against high wall time is the signature of subprocess spawning,
not parsing. `doctor.rs:140`, `doctor.rs:191`, `doctor.rs:232` and
`validate.rs:231` all loop over per-file readers that reload config. Phase 1
fixes the CLI and the server with the same change.

### 2.4 Payload duplication

| `RepositorySnapshot` section | Approx. serialized size |
|---|---:|
| Top-level `stories` | 267 KB |
| Stories cloned into `epics[].stories` | 272 KB |
| Stories cloned into `sprints[].storiesByStatus` | 147 KB |
| `progress` (all the app header uses) | 500 bytes |
| Active sprint only (all the board uses) | 43 KB |
| Compact epic metadata only (all the dashboard uses) | 4.5 KB |

A story is cloned into an epic only when `story.epic` is `Some`
(`snapshot.rs:143`) and into a sprint only when its sprint matches and its
status is in `BOARD_STATUSES` (`snapshot.rs:169-179`). So three copies is the
upper bound, not the invariant. `WebStory` carries `tasks: Vec<WebTask>` with
full descriptions and a complete `frontmatter` map (`dto.rs:73-77`), so each
copy is expensive.

**Note for fixture design:** this repository has `"sprints": false` in
`.kanban/settings.json`, so the sprint clone is zero here and the observed
multiplier is ~2x, not 3x. See §5.3.

---

## 3. Confirmed Root Causes

All citations below were verified against the working tree at revision 2.

### 3.1 Configuration and Git root resolution occur per file

`read_repository` (`crates/core/src/repository.rs:317-323`) loads config, then
calls `collect_user_story_files` and `read_story_file` — **both of which load
config again**:

- `repository.rs:317` — `load_kanban_config(repo_root)`
- `repository.rs:44` — `collect_user_story_files` → `load_kanban_config`
- `repository.rs:207` — `read_story_file` → `load_kanban_config`, once per story
- `repository.rs:88` — `collect_epic_files` → `load_kanban_config`

Every `load_kanban_config` (`config.rs:346-349`) calls `resolve_repo_root`
(`config.rs:296-303`) which spawns `git -C <path> rev-parse --show-toplevel`
(`config.rs:535-551`), and then re-reads and re-parses `.kanban/settings.json`
(`config.rs:499-513`).

**There is no memoization anywhere.** A repository-wide search for
`OnceLock|OnceCell|lazy_static|once_cell` finds only two test-only locks
(`self_manage.rs:339`, `typegen.rs:152`). Neither `kanban-core` nor
`kanban-web-server` depends on `once_cell` or `dashmap`.

Process sampling attributed ~96% of backend time beneath `load_kanban_config`.

### 3.2 Epic construction is a full-repository N+1

`load_epics` (`snapshot.rs:117-140`) reads each epic file, computes
`source_overview`, and then calls `find_epic` for the same epic:

- `snapshot.rs:121` — `kanban_core::epic_overview(&source)`
- `snapshot.rs:122` — `find_epic(repo_root, &source_overview.id)`
- `epic.rs:31` — `find_epic` → full `read_repository`
- `epic.rs:93-98` — `find_epic_source` → `collect_epic_files` + `read_epic_file`
  for **every** epic file, again

Two details revision 1 missed:

1. `find_epic_source` rescans all epic files on top of the full
   `read_repository`, so the loop is doubly quadratic.
2. `source_overview` is consumed **only** for `.id` at line 122; every other
   field comes from `details.epic`. The loop already holds `source` and can
   build `WebEpic` from it with zero additional reads.

### 3.3 Sprint, metrics, and report paths reread the repository

- `load_sprints` (`snapshot.rs:164`) → `summarize_sprints` (`sprint.rs:41`) →
  full `read_repository`; `summarize_sprints_from_repository` (`sprint.rs:293`)
  loads config once more.
- `api_metrics` (`handlers/mod.rs:126-128`) builds the full snapshot, then calls
  `list_all_stories` **and** `summarize_sprints` — two more full reads.
- `api_report` (`handlers/mod.rs:139-141`) repeats the identical pattern.
- `compute_metrics` (`metrics.rs:100`) recomputes `compute_progress` even though
  `load_repository_snapshot` already stored `progress` (`snapshot.rs:27, 32`).

### 3.4 Cost model

The observed request costs are exactly derivable. With `S` stories and `E`
epics, counting `load_kanban_config` calls:

```
read_repository      = 2 + S                      (top + collector + per story)
find_epic            = read_repository + 1        (+ find_epic_source collector)
load_epics           = 1 + E * find_epic
load_sprints         = read_repository + 1
snapshot             = read_repository + load_epics + load_sprints
/api/metrics         = snapshot + read_repository + load_sprints
```

For S=214, E=27:

| Path | Config loads / git spawns | Est. wall @4.06 ms |
|---|---:|---:|
| `read_repository` | 216 | 0.9 s |
| `load_epics` | 5,860 | 23.8 s |
| `load_sprints` | 217 | 0.9 s |
| **`/api/repository`** | **6,293** | **25.5 s** |
| **`/api/metrics`** | **6,726** | **27.3 s** |
| Cold dashboard (both) | 13,019 | — |

These match the measured 18.4-23.2 s within the error of spawn-cost estimation.
**`load_epics` is 93% of the cost.** Fixing §3.2 alone is the single highest-value
change in this document.

### 3.5 Epic detail is the worst endpoint

`load_epic_detail` (`snapshot.rs:102-112`) calls `load_repository_snapshot`
(already ~6,293 config loads) and then `find_epic_with_source` (`epic.rs:38-41`),
another full read. `GET /api/epics/{id}` is measurably worse than
`/api/repository` and was not listed in revision 1.

### 3.6 Every request builds from scratch

`AppState` (`lib.rs:43-58`) holds only `branch_cache`. Repository, metrics,
report, and epic handlers each build independently. `spawn_blocking` work cannot
be cancelled, so an abandoned browser reload leaves the expensive build running.

### 3.7 The frontend requests more data than each page needs

- `AppShell.tsx:8` calls `useRepository()`; the only consumption is
  `repo.data.progress` at line 27 — four numbers out of a 687 KB payload.
- `BoardView.tsx:43` reads `repo.data!.sprints` and nothing else.
- `DashboardView.tsx:156` calls `useRepository()` and uses only
  `repository.data?.epics` at line 241. It does not even check
  `repository.isLoading` or `.error`, so the dashboard blocks on metrics but
  silently renders empty phases if the repository call fails.

### 3.8 Live-reload is unbounded, lossy, and has no client error path

This is a correctness defect, not only a performance one.

**Server:** the watcher (`lib.rs:220-227`) fires `events.send(())` on **every
raw `notify` event** with no debouncing. One `git pull` or a multi-file
`kanban` write fans out into many `change` events.

**Client:** `useLiveReload` (`hooks.ts:148-156`) invalidates three query keys per
event with no throttle. Combined with the server, one `git pull` triggers dozens
of full 687 KB refetches per connected client.

**Reliability holes that make revision 1's `staleTime: Infinity` unsafe:**

- `api_events` emits `Event::default().event("change").data("{}")`
  (`handlers/mod.rs:477`) — **no event `id`**, and no `Last-Event-ID` handling.
  `EventSource` reconnects automatically, but every change during the gap is
  lost permanently.
- `RecvError::Lagged(_) => continue` (`handlers/mod.rs:481`) silently discards
  events for a slow subscriber.
- Exceeding `SSE_SUBSCRIBER_CAP` returns `503` (`handlers/mod.rs:464-467`), and
  `useLiveReload` registers **no `onerror` handler**. A capped-out client gets
  no live reload and no indication that it lost it.

Today `staleTime: 0` masks all three: the next focus or mount self-heals. Making
SSE the sole freshness mechanism converts each into permanent silent staleness
on a tool whose entire value is showing current state. See WP-05.

### 3.9 Static assets are uncacheable, uncompressed, and copied per request

`static_asset` (`handlers/mod.rs:491-511`) sets only `Content-Type`. A search
for `Cache-Control|ETag|Accept-Ranges` across `crates/web-server/src` returns
zero matches. There is no compression middleware — `tower-http` is **not a
workspace dependency**; `tower` exists only in `crates/web-server`'s
`[dev-dependencies]`.

Additionally `file.contents().to_vec()` (`handlers/mod.rs:502`) allocates a
fresh copy of every asset on every request, fixable independently with
`Bytes::from_static`.

Existing story `US-034` (`status: draft`) already owns cache headers, ETag, and
range support.

### 3.10 Per-card query observers

`useAssigneeMap` (`StoryCard.tsx:44-53`) calls `useTeam()` inside `CardContent`,
which renders once per card (`StoryColumn.tsx:37-39`). `useTeam` has
`staleTime: 5 min` (`hooks.ts:56`) so this costs no extra network, but it
creates one `QueryObserver` subscription and one `Map` allocation per card, and
every team-cache update notifies all of them.

---

## 4. Goals and Budgets

### 4.1 Functional goals

- Preserve all current board, dashboard, backlog, sprint, report, mutation,
  optimistic-update, and live-reload behavior.
- Preserve markdown files as the authoritative state.
- Guarantee generation coherence: repository, progress, metrics, and report data
  served together must derive from the same source read.
- Keep blocking filesystem work inside `spawn_blocking` (preserves `US-023`).
- Preserve `RepoLock`, `AppState::write_lock`, and `ensure_path_inside`
  guarantees exactly.
- Never leave a client silently showing stale data after a successful mutation,
  external edit, or `git pull`.

### 4.2 Budgets

| # | Metric | Target | How asserted |
|---|---|---:|---|
| B1 | `git rev-parse` spawns per `read_repository` | **1** | Unit test, injected counter |
| B2 | `git rev-parse` spawns per web read-model build | **1** | Unit test, injected counter |
| B3 | Complete story parses per read-model build | **1** | Unit test, injected counter |
| B4 | `settings.json` parses per read-model build | **1** | Unit test, injected counter |
| B5 | Read-model build, 250-story fixture | p95 <= 250 ms | WP-01 benchmark |
| B6 | `kanban doctor` on 250-story fixture | <= 500 ms | WP-01 benchmark |
| B7 | Rebuilds per filesystem event burst | **1** | Deterministic test, fake clock |
| B8 | SSE `change` events per burst | **1** | Deterministic test, fake clock |
| B9 | Board cold LCP, warm server, cold browser cache | <= 1000 ms | Gate G1 trace |
| B10 | Dashboard cold LCP, warm server, cold browser cache | <= 1000 ms | Gate G1 trace |
| B11 | Repeat-load bytes for unchanged hashed assets | **0** | Header test + trace |
| B12 | CLS from loading-state replacement | <= 0.02 | Gate G1 trace |

Conditional on gate G1 only:

| # | Metric | Target |
|---|---|---:|
| B13 | Prewarmed API server processing | p95 <= 20 ms |
| B14 | Board JSON payload, uncompressed | <= 100 KB |
| B15 | Dashboard JSON payload, uncompressed | <= 25 KB |
| B16 | Progress JSON payload | <= 2 KB |

B1-B4 and B7-B8 are exact, deterministic, and must be enforced by the normal
test suite — they are the regression guards. B5, B6, B9, B10, B12 are release
checks measured by the WP-01 harness, never asserted in CI unit tests.

B9/B10 were relaxed from revision 1's 500 ms because 500 ms is not meaningfully
better than 1000 ms for a local tool, and chasing it is what motivated the
conditional Phase 3 work. B12 was tightened from 0.1 (the "needs improvement"
boundary) because fixed-dimension skeletons should produce ~0.

---

## 5. Constraints, Non-Goals, and Fixture Requirements

### 5.1 Constraints

- No database, no generated on-disk index, no persisted cache.
- Do not bypass `RepoLock` or `AppState::write_lock`.
- Keep backlog semantics in `crates/core`.
- Do not undo `spawn_blocking` around blocking repository work.
- Never hand-edit `web/shared/generated/api.ts`.
- No unit test may depend on the external IP 2.0 checkout.
- No wall-clock timing assertions in the normal test suite.

### 5.2 Non-goals

- Persisting cache between restarts.
- Incremental single-file reparse in this project.
- Replacing React Query, React Router, or Recharts.
- Server-side rendering.
- Visual redesign.
- Removing `/api/repository` while backlog and sprint views consume it.
- Service workers.

### 5.3 Fixture requirements (binding on WP-01)

Revision 1 specified fixture size but not feature flags. Because this repository
runs `"sprints": false`, a fixture that inherits local defaults would **not
exercise the code path that costs 93% of the time in §3.4**.

The representative fixture must be generated with:

```json
{ "paths": { "backlog": "delivery/backlog", "sprints": "delivery/sprints" },
  "features": { "phases": false, "sprints": true, "epics": true } }
```

- 250 stories, 30 epics, 5 sprints, ~180 sibling `.tasks.md` files.
- A realistic status distribution across all `BOARD_STATUSES` plus at least one
  status alias.
- At least one story with no `epic`, one with an `epic` that has no epic file,
  one with a referenced `task_file`, and one with a sibling `.tasks.md`.
- Generated into a `tempdir` and `git init`-ed, so `resolve_repo_root` exercises
  the real subprocess path. Never committed.

A second minimal fixture must pin `features: { phases: false, sprints: false,
epics: true }` to match this repository, so both configurations stay covered.

---

## 6. Target Architecture

Phase 1 target — no caching, one pass:

```text
markdown files
     |  one config load, one repository parse
     v
RepositorySource { Repository, KanbanConfig, sprint overviews }
     |  one derivation pass, no re-reads
     v
WebReadModel  { progress | snapshot | metrics | report | epic index }
     |
     v
HTTP handlers (build per request, inside one spawn_blocking)
```

Phase 3 target — only if gate G1 fails:

```text
     WebReadModel (generation N)
          |  atomic Arc swap after successful rebuild
          v
     AppState read-model cache  -->  HTTP handlers
```

Change handling, from Phase 1 onward:

```text
filesystem event or successful mutation
  -> coalesce burst behind a debounce window
  -> (Phase 3 only) build generation N+1 in spawn_blocking
  -> (Phase 3 only) atomically publish; discard and rebuild if superseded
  -> emit exactly one SSE change event, carrying a monotonic id
  -> clients invalidate the affected keys once
```

Debouncing and single-SSE-per-burst are **Phase 1**, because they are correct
and valuable with or without a cache.

---

## 7. Phase Plan

| Phase | Package | Ownership | Depends on | Mandatory | Model |
|---|---|---|---|---|---|
| 1 | WP-01 | Benchmark harness, fixture generator, baseline | — | Yes | Sonnet |
| 1 | WP-02 | Single config resolution in `crates/core` | WP-01 for evidence | Yes | **Opus** |
| 1 | WP-03 | One-pass web read model | WP-02 | Yes | **Opus** |
| 1 | WP-04 | Watcher debounce + SSE identity/reliability | — | Yes | **Opus** |
| — | **G1** | **Measurement gate** | WP-01..04 | **Yes** | **Opus** (decision) |
| 2 | WP-05 | Frontend query policy, bundle, render polish | WP-04 | Yes | Opus → Sonnet |
| 2 | WP-06 | Static asset delivery (`US-034`) | — | Yes | Sonnet |
| 3 | WP-07 | Page-specific API contracts | G1 fail, WP-03 | Conditional | Opus → Sonnet |
| 3 | WP-08 | Prewarmed read-model cache | G1 fail, WP-07 | Conditional | **Opus** |
| 4 | WP-09 | Integrated verification and release evidence | all landed | Yes | Sonnet → Opus |

Parallelism: WP-01, WP-02, WP-04, and WP-06 can start immediately and touch
disjoint files. WP-03 needs WP-02's APIs. WP-05 needs WP-04's SSE contract.

Every package bumps the workspace version per `AGENTS.md` and runs the full
verification suite before handoff.

### 7.1 Model Selection Policy

Tier is chosen by **blast radius and verifiability**, not by diff size. The
governing question is: *if the model gets this subtly wrong, will the test suite
and the compiler catch it?*

- If a mistake is caught by `cargo build` or an existing assertion → **Sonnet**.
- If a mistake produces output that still compiles, still passes existing tests,
  and silently corrupts backlog data or serves stale state → **Opus**.
- If the task is a templated transformation with a worked example already in the
  repository → **Haiku**.

This is why WP-02 is Opus despite being the most mechanical-looking package in
the plan. Threading `&KanbanConfig` through collectors and readers is
compiler-guided, but the failure mode is a silent change to path containment,
symlink handling, or story ordering in a markdown backlog that is the user's
only source of truth. `AGENTS.md` explicitly forbids silent rewrites of backlog
documents; that constraint cannot be delegated to a cheaper tier.

Conversely WP-06 is Sonnet despite touching HTTP correctness, because RFC 9110
range and validator semantics are well-specified, the acceptance criteria are
already enumerated in `US-034`, and every failure mode is directly assertable in
a unit test.

**`Opus → Sonnet`** means: Opus produces the design artifact (the query policy,
the DTO contract, the interpretation of results), Sonnet executes the mechanical
remainder against that frozen artifact. Split at the handoff boundary in §15.

#### Escalation and downgrade rules

Escalate to Opus mid-package when any of these occur:

1. A golden-output or equivalence test fails and the cause is not immediately
   obvious from the diff.
2. The implementation requires changing a public `crates/core` signature not
   named in the package's step list.
3. A deterministic concurrency test is flaky.
4. The work requires deciding *whether* an existing behavior was intentional.
5. Two packages conflict on a shared file (`lib.rs`, `handlers/mod.rs`).

Downgrade to Sonnet or Haiku only after the design decision is written down in
this document or the owning backlog story. A cheaper tier executing a recorded
decision is safe; a cheaper tier *making* the decision is not.

#### Review tiering

Review tier must be at least the implementation tier, and reviews of Opus-tier
work should be performed by a *fresh* Opus context rather than the implementing
session — the failure modes in WP-02, WP-03, and WP-08 are precisely the ones an
author is least likely to re-examine. The §15 handoff artifact exists to make
that fresh-context review possible.

#### Where Haiku genuinely fits

Haiku is not suitable for any whole package here, but is appropriate for these
discrete sub-tasks, each of which is fully verified by a command:

| Sub-task | Package | Verified by |
|---|---|---|
| `Bytes::from_static` swap at `handlers/mod.rs:502` | WP-06 | `cargo build` + existing asset test |
| Regenerate TypeScript bindings | WP-07 | `generated_bindings_are_current` |
| Workspace version bump per `AGENTS.md` | all | `cargo build` |
| Author `US-042`..`US-046` files from the §12 table | §12 | `kanban validate .` |
| Run the §16 verification suite and report failures verbatim | WP-09 | exit codes |
| Mechanical test-fixture boilerplate from a worked example | WP-01 | `cargo test` |

Do not use Haiku to *interpret* a failure from any of these — only to run them
and report.

#### Routing mechanism

Tier is not a suggestion attached to a prompt; it is enforced by routing to a
model-bound subagent. `impl-sonnet`, `runner-haiku`, and `review-opus` are
configured in `~/.config/opencode/opencode.json`. See §15.1 for the mapping and
for how the routing is recorded.

Two consequences follow from model binding being per-agent rather than per-call:

1. An `Opus → Sonnet` split is two invocations with a written artifact between
   them. If the design half cannot be reduced to a brief that `impl-sonnet` can
   execute without judgment, the package is not actually splittable and should
   run entirely at Opus.
2. Subagents start with no context from this conversation. The §15 handoff
   requirements are what make a brief self-contained, which is why they are
   mandatory rather than clerical.

---

## 8. Phase 1 — Eliminate the Algorithmic Blowup (mandatory)

### WP-01: Reproducible Performance Harness and Fixtures

**Model: Sonnet.** Scripting and fixture generation with directly verifiable
output. One exception: **step 7 (the injectable resolution counter) is Opus** —
it defines the assertion mechanism for B1-B4, and a counter placed at the wrong
call site would make every subsequent package's central acceptance criterion
vacuously true.

**Objective:** Make every later claim measurable without DevTools sessions or one
developer's machine state.

**Files:** `scripts/benchmark_web_load.py` (a `scripts/` directory with Python
tooling already exists — reuse it rather than introducing shell percentile
math); a fixture generator in a shared test-support module usable by both
`crates/core` and `crates/web-server`.

**Steps:**

1. Fixture generator per §5.3, both configurations, into a `git init`-ed
   `tempdir`. Expose it as a test-support helper, not a committed fixture tree.
2. Benchmark accepting `--base-url`, `--runs` (default 20), `--warmup`,
   `--output {json,csv,table}`.
3. Measure each endpoint separately: `/api/repository`, `/api/metrics`,
   `/api/report`, `/api/epics/{id}`, and (if Phase 3 lands) `/api/progress`,
   `/api/board`.
4. Record status, response bytes, TTFB, and total time per run; report min,
   median, p95, max.
5. Include a concurrent board+dashboard scenario, which is what exposes
   duplicate builds.
6. Add a no-HTTP mode measuring a cold read-model build and `kanban doctor`
   directly, so B5 and B6 are separable from HTTP overhead.
7. **Add an injectable resolution counter.** Put a `#[cfg(test)]`
   `AtomicUsize` behind the `git_toplevel` and `read_settings` call sites in
   `crates/core/src/config.rs`, exposed through a test-only accessor. This is
   what makes B1-B4 real assertions rather than aspirations, and it is the only
   durable guard against this regression returning. Do not add production
   global mutable state.
8. Document the LCP trace procedure exactly: fresh browser context, DevTools
   cache disabled for cold runs and enabled for repeat runs, `about:blank`,
   start recording, navigate once.

**Acceptance:**

- One command reproduces endpoint timing and payload size, n>=20, with p95.
- Cold build and prewarmed hit are reported separately.
- The concurrent scenario is represented.
- Works with no external repository.
- The counter is available to WP-02 and WP-03 tests.

**Handoff:** Land before any optimization commit, and publish a baseline run
against the §5.3 fixture so later before/after numbers are comparable.

---

### WP-02: Single Configuration Resolution in Core

**Model: Opus.** Highest blast radius in the plan. The refactor is
compiler-guided, but the failure modes — altered path containment, symlink
escape behavior, story ordering, or task-file resolution — compile cleanly and
can pass existing tests while silently corrupting the user's only source of
truth. Risk R1. Do not delegate the equivalence-test design or the
`ensure_path_inside` review to a cheaper tier. Once the config-aware signatures
are frozen and the golden test is green, propagating the remaining CLI call
sites (`dispatch.rs:523`, `doctor.rs:140/191/232`) is Sonnet work.

**Objective:** One `read_repository` resolves the repository root and parses
`.kanban/settings.json` exactly once. This also fixes `validate` and `doctor`.

**Files:** `crates/core/src/config.rs`, `repository.rs`, `sprint.rs`,
`story.rs`, `doctor.rs`, `validate.rs`, `epic.rs`; `crates/cli/src/dispatch.rs`.

**Design decision — resolve this first.** Two viable approaches:

| Approach | Effort | Risk | Verdict |
|---|---|---|---|
| **A. Thread `&KanbanConfig`** through config-aware variants of the collectors and readers | ~300 lines across core + 4 CLI call sites | Low; compiler-enforced | **Chosen** |
| B. Memoize `resolve_repo_root`/`load_kanban_config` in a process-local map keyed by canonical path, invalidated on `settings.json` mtime | ~30 lines | Correctness depends on mtime granularity; hidden coupling; surprising in a library crate | Rejected |

B is tempting and is *not* forbidden by §5.1 (which bars persisted on-disk
caches). It is rejected because a library crate that silently caches filesystem
state is a latent bug for the CLI's `config set` path and for tests that mutate
`settings.json` within one process. Approach A makes the data flow explicit,
which §"Development Rules" in `AGENTS.md` requires. Record this in an ADR under
`delivery/decisions/` if the reviewer disagrees.

**Steps:**

1. Keep every existing public convenience API working for CLI callers.
2. Add config-aware variants:
   - `collect_user_story_files_with_config(&KanbanConfig)`
   - `collect_epic_files_with_config(&KanbanConfig)`
   - `read_story_file_with_config(path, &KanbanConfig)`
   - `read_epic_file_with_config(path, &KanbanConfig)`
3. Rewrite `read_repository` to load config once and use the `_with_config`
   forms for all files.
4. Add `summarize_sprints_from_repository(&Repository, &KanbanConfig)` as the
   public non-reloading entry point; make `summarize_sprints` a thin wrapper.
   Keep web concepts out of core.
5. Apply the same treatment to the CLI loops: `doctor.rs:140`, `doctor.rs:191`,
   `doctor.rs:232`, `doctor.rs:564`, `validate.rs:231`, `story.rs:195`.
6. Ensure `ensure_path_inside` and `validate_task_file_frontmatter_value` reuse
   the supplied config; containment behavior must be byte-identical.
7. Retire `find_epic_source`'s redundant rescan by adding a config-aware form
   that accepts an already-collected epic file list.

**Tests:**

- **B1/B4 as hard assertions**: `read_repository` on the §5.3 fixture performs
  exactly one root resolution and one settings parse, via the WP-01 counter.
- Byte-for-byte equivalence of parsed story fields, task files, ordering, and
  paths before and after (golden-output test against the fixture).
- Referenced and sibling task-file containment unchanged, including the existing
  symlink-escape cases (`validate.rs:1150`, `validate.rs:1184`).
- Duplicate IDs and malformed files behave identically.
- `doctor` and `validate` CLI output is unchanged on both fixture
  configurations.

**Acceptance:** B1, B4, and B6 met. No config load inside any per-file loop in
`crates/core`. No public API removed.

**Boundary:** No React, no web DTOs, no cache state. Ends with reusable core
APIs and zero web behavior change.

---

### WP-03: One-Pass Web Read Model

**Model: Opus.** The package must preserve exact equivalence across five
projections (snapshot, epic grouping, sprint buckets, metrics, report) while
restructuring how all of them are derived. Deleting the `find_epic` call at
`snapshot.rs:122` is one line; determining that `source` carries every field
`details.epic` supplied, and that the absent-epic-file fallback still behaves
identically, is the actual work. Risk R2. Metrics equivalence (burnup,
burndown, lead time, velocity, forecast) is the specific area where a plausible
but wrong derivation will pass a shallow test.

**Objective:** Build repository, epic, sprint, metrics, and report projections
from a single parsed `Repository`.

**Files:** new `crates/web-server/src/read_model.rs`; `snapshot.rs`,
`metrics.rs`, `metrics/`, `handlers/mod.rs`.

**Steps:**

1. Define `RepositorySource { repository: Repository, config: KanbanConfig,
   sprints: Vec<SprintOverview>, stories_by_epic: HashMap<String, Vec<usize>> }`
   — index by normalized epic ID into the story vector, do not clone stories.
2. Add one pure builder `build_web_read_model(&Path) -> Result<WebReadModel>`
   calling `read_repository` exactly once.
3. **Delete the `find_epic` call at `snapshot.rs:122`.** Build `WebEpic`
   directly from the already-read `source` (`snapshot.rs:120`) plus the epic
   index. This single change removes 93% of the cost per §3.4.
4. Build sprint overviews via `summarize_sprints_from_repository`, not
   `summarize_sprints(repo_root)`.
5. Derive `StoryOverview` from the parsed stories instead of `list_all_stories`.
6. Compute metrics and report from the same source. Remove the duplicate
   `compute_progress` at `metrics.rs:100`; make `WebReadModel.progress` the
   single canonical value and have `compute_metrics` accept it.
7. Rewrite `load_epic_detail` (`snapshot.rs:102-112`) to serve `GET
   /api/epics/{id}` from one source build with no second read.
8. Rewrite `api_metrics` (`handlers/mod.rs:126-128`) and `api_report`
   (`handlers/mod.rs:139-141`) to build the source once.
9. Clone full stories into DTOs only where the current wire format requires it;
   keep the internal model index-based.
10. Keep the whole build inside one `spawn_blocking` when called from async.

**Tests:**

- **B2/B3 as hard assertions** via the WP-01 counter, for `/api/repository`,
  `/api/metrics`, `/api/report`, and `/api/epics/{id}`.
- Golden-output equivalence for the full `RepositorySnapshot`: stories, epic
  grouping, sprint buckets, ordering, and progress, on both fixture
  configurations.
- Metrics equivalence: burnup, burndown, lead time, velocity, forecast.
- Report equivalence against existing report fixtures.
- Epic detail returns identical source and child stories.
- A story whose epic file is absent keeps the current fallback behavior.
- Status aliases still map into the same board buckets.

**Acceptance:** B2, B3, B5 met. `find_epic` is not called from `load_epics`.
No handler calls `list_all_stories` or `summarize_sprints` after the source is
loaded. Wire format is unchanged — this package is a pure refactor observable
only as latency.

**Boundary:** No long-lived cache state. No DTO shape changes. Make one uncached
build fast and provably correct before anyone adds lifecycle complexity.

---

### WP-04: Watcher Debouncing and SSE Identity

**Model: Opus.** Concurrency plus a wire protocol. Debounce-with-ceiling,
`broadcast::RecvError::Lagged` recovery, monotonic generation counters, and
`Last-Event-ID` resumption interact in ways that are easy to implement
plausibly and wrong. The tests must use injected time rather than sleeps, which
is itself a design decision. A subtle error here manifests as a client that is
silently stale — the exact defect this package exists to prevent. Risk R3, R4.

**Objective:** One source-change burst produces one SSE event, and clients can
detect that they missed events. This is a defect fix and a prerequisite for
WP-05's query policy.

**Files:** `crates/web-server/src/lib.rs`, `crates/web-server/src/handlers/mod.rs`.

**Steps:**

1. Replace the direct `events.send(())` in the watcher closure
   (`lib.rs:220-227`) with a send into a bounded coalescing channel.
2. Add a debounce task: on first event start a timer (default 150 ms, ceiling
   1 s for sustained bursts such as `git pull`), and emit exactly one
   downstream notification when it expires. Make the interval configurable for
   tests via injected time, not by sleeping.
3. Change the broadcast payload from `()` to a monotonic `u64` generation
   counter incremented once per coalesced burst.
4. Emit SSE with that counter as the event id:
   `Event::default().id(gen.to_string()).event("change").data(...)`.
5. Handle `Last-Event-ID` on reconnect: if the client's id is behind the current
   counter, immediately emit one `change` event so the client resynchronizes.
   This closes the reconnect-gap hole in §3.8.
6. Replace `RecvError::Lagged(_) => continue` (`handlers/mod.rs:481`) with an
   explicit resynchronization event carrying the current counter, so a lagged
   subscriber is told to refetch rather than silently losing changes.
7. Keep `branch_cache` invalidation on the raw event, before debouncing — it is
   cheap and must not be delayed.
8. Route mutation and `git pull` handlers through the same coalescing path
   rather than calling `state.events.send(())` directly.
9. Also watch `.kanban/settings.json`, since config changes affect served data.

**Tests (deterministic, injected clock — never real sleeps):**

- A burst of 50 raw events within the window produces exactly one SSE event
  (B7, B8).
- Sustained events beyond the ceiling still emit at the ceiling interval.
- The generation counter is strictly monotonic and never repeats.
- A reconnect with a stale `Last-Event-ID` receives an immediate `change`.
- A lagged subscriber receives a resync event rather than silently continuing.
- A successful mutation emits exactly one event.
- A failed or no-op mutation emits none.
- `branch_cache` is still invalidated promptly.

**Acceptance:** B7 and B8 met. No code path calls `events.send` outside the
coalescer. Every SSE event carries an id.

---

## 9. Gate G1 — Measurement Decision Point

**This gate is mandatory and blocks Phase 3.**

**Model: Sonnet to collect, Opus to decide.** Running the harness and capturing
traces is mechanical. Interpreting a partial result — B5 met but B9 missed, or
a p95 that is close to budget with high variance — determines whether roughly a
thousand lines of conditional work get built, and is the highest-leverage single
judgment in the plan. Risk R7 is specifically the failure to take this decision
seriously.

### 9.1 Result — measured 2026-08-04, **G1 PASSED**

Environment: release build, macOS, loopback, no CPU or network throttling.
Fixture: `FixtureSpec::representative()` per §5.3 — 250 stories, 30 epics,
5 sprints, 180 sibling task files, `features: { phases: false, sprints: true,
epics: true }`, generated into a `git init`-ed tempdir.

Server measurements: `python3 scripts/benchmark_web_load.py --runs 20`.
Build measurements: `cargo test -p kanban-web-server --release -- --ignored
--nocapture read_path_bench`. Browser measurements: single clean navigation per
§WP-01 step 8, warm server, cold browser cache.

| Measurement | Before (§2.2, 214 stories) | After (250 stories) | Budget | Verdict |
|---|---:|---:|---:|---|
| `/api/repository` p95 | 18,400-19,500 ms | **47.0 ms** | — | — |
| `/api/metrics` p95 | 23,200 ms | **45.6 ms** | — | — |
| `/api/report` p95 | not measured | **48.3 ms** | — | — |
| `/api/epics/{id}` p95 | worse than repository | **40.2 ms** | — | — |
| Concurrent board+dashboard p95 | 21,800-23,200 ms | **53.7 ms** | — | — |
| Read-model build p95 | — | **42.3 ms** | B5 <= 250 ms | **pass** |
| `kanban doctor` p95 (250-story fixture) | — | **51.7 ms** | B6 <= 500 ms | **pass** |
| Board cold LCP, warm server | 22,900 ms | **167 ms** | B9 <= 1000 ms | **pass** |
| Dashboard cold LCP, warm server | 24,500 ms | **235 ms** | B10 <= 1000 ms | **pass** |
| Board CLS | — | **0.01** | B12 <= 0.02 | **pass** |
| Dashboard CLS | — | **0.01** | B12 <= 0.02 | **pass** |

This repository's own CLI, release build, 41 stories:

| Command | Before (§2.3) | After |
|---|---:|---:|
| `kanban validate .` | 0.56 s | **0.02 s** |
| `kanban doctor .` | 0.87 s | **0.02 s** |

**Decision: stop. Phase 3 is not built.**

B5, B9 and B10 are all met with two to six times of headroom, so the §9 decision
rule says stop unconditionally. WP-07 (page-specific API contracts) and WP-08
(prewarmed read-model cache) are moved to §14 Deferred. Building a
generation-tracking, supersede-aware, atomically-swapped cache to remove a cost
that is now 42 ms would add roughly a thousand lines of stale-data surface for
no measurable user benefit, which is exactly what gate G1 exists to prevent
(risk R7).

The §3.4 cost model is confirmed: `load_epics` was 93% of the cost, and deleting
one `find_epic` call plus threading `&KanbanConfig` removed it. Parsing was never
the bottleneck, so WP-08 would not have helped even if a budget had been missed.

### 9.2 Residual observations recorded at the gate

- `/api/repository` is 947 KB uncompressed for the 250-story fixture, but
  transfers in ~1 ms on loopback and deserializes inside the 167 ms board LCP.
  Payload splitting (WP-07) is therefore not justified; if it ever is, §14's
  generation-based `ETag` is the cheaper first move now that WP-04 supplies a
  monotonic generation.
- Lighthouse's cache insight reported ~417 kB (board) and ~667 kB (dashboard) of
  re-transferred assets per repeat visit. That is WP-06, which is mandatory and
  independent of this gate.
- Unknown `/api/*` paths returned `200` with `index.html` rather than `404`.
  Confirmed during this measurement and fixed in WP-06 step 10.

### 9.3 Original gate template

## 10. Phase 2 — Client Correctness and Delivery (mandatory, independent of G1)

### WP-05: Frontend Query Policy, Bundle, and Render Polish

**Model: Opus → Sonnet.** Split at the policy boundary.

*Opus:* the `QueryClient` defaults and their interaction with WP-04's SSE
contract (steps 1-5). React Query's `staleTime`/`refetchOnMount`/
`refetchOnWindowFocus`/invalidation interplay is the single place in this plan
where a confident-looking wrong answer reintroduces silent staleness — which is
precisely the defect that revision 1 shipped. Also Opus: deciding what the
`onerror` fallback does and when it disengages.

*Sonnet:* everything mechanical against the frozen policy — hoisting `useTeam`
out of `StoryCard` (step 6), the `React.lazy` conversion (step 7),
`DashboardView` error handling (step 8), skeletons and `keepPreviousData`
(steps 9-10), and the corresponding Vitest updates.

**Objective:** Stop redundant refetches without introducing silent staleness;
remove avoidable client work.

**Files:** `web/src/api/hooks.ts`, `web/src/main.tsx`,
`web/src/components/AppShell.tsx`, `StoryCard.tsx`, `StoryColumn.tsx`,
`web/src/views/BoardView.tsx`, `BacklogView.tsx`, `DashboardView.tsx`,
`web/src/styles/app.css`, corresponding Vitest files.

**Query policy — this replaces revision 1's `staleTime: Infinity`.**

Revision 1 proposed `staleTime: Infinity` plus disabled focus/mount refetch,
making SSE the sole freshness mechanism. Per §3.8 that is unsafe. Adopt instead:

```ts
// web/src/main.tsx
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,          // SSE handles latency; this is the safety net
      refetchOnWindowFocus: true, // cheap once the server responds in ~20 ms
      refetchOnMount: false,      // remount within staleTime serves from cache
      refetchOnReconnect: true,
    },
  },
});
```

A 60 s bounded staleness plus SSE gives the same practical freshness as
`Infinity` while degrading gracefully when SSE is lost to a reconnect gap, a
lagged subscriber, or the subscriber cap. Once the server answers in tens of
milliseconds, a refetch on focus costs nothing worth optimizing away.

**Steps:**

1. Set the `QueryClient` defaults above; delete the per-hook `staleTime` on
   `useConfig` only if the new default is not weaker (it is — keep `Infinity`
   there, config rarely changes).
2. Add an `onerror` handler to `useLiveReload` (`hooks.ts:148-156`) that
   surfaces a non-blocking "live updates unavailable" indicator and falls back
   to a 30 s `refetchInterval` for aggregate queries until SSE recovers.
3. Track the SSE event id from WP-04 and pass `Last-Event-ID` implicitly via
   `EventSource` reconnect; verify resync events invalidate correctly.
4. Debounce `useLiveReload` invalidation to one batch per animation frame as a
   client-side backstop, even though WP-04 coalesces server-side.
5. Replace the unfiltered `queryClient.invalidateQueries()` after `gitPull`
   (`hooks.ts:48`) with explicit keys: `["repository"]`, `["metrics"]`,
   `["report"]`, `["team"]`. Today it also blows away `["config"]` and every
   `["story", id]`.
6. Hoist `useTeam` out of `StoryCard` (§3.10). Resolve the assignee map once in
   `BoardView`/`StoryColumn` and pass resolved data down as props.
7. Convert the `StoryModal` import in `BoardView.tsx:18` and
   `BacklogView.tsx:20` to `React.lazy` + dynamic `import()`. Note the accurate
   framing: StoryModal (58 KB) and `purify.es` (70 KB) are **not** in the entry
   chunk — they are static dependencies of the board and backlog route chunks,
   so they load on first board visit even if no card is opened. Deferring them
   removes ~129 KB from board startup.
8. Add `DashboardView.tsx` error/loading handling for its data source; today it
   checks only `metrics` (§3.7).
9. Replace `Loading...` (`BoardView.tsx:40`) with fixed-dimension skeletons
   matching the board columns and dashboard chart cards, with
   `role="status"`/`aria-busy`.
10. Keep the last-known view during background refresh instead of unmounting to
    a loading state (`placeholderData: keepPreviousData`).
11. Optionally prefetch the dashboard route chunk on link hover or `requestIdleCallback`.
12. Do not add `useMemo`/`useCallback` speculatively.

**Tests:**

- Remount within `staleTime` does not refetch.
- Window focus after `staleTime` does refetch.
- One SSE `change` invalidates each aggregate key exactly once.
- SSE `onerror` enables the polling fallback and the indicator; recovery
  disables both.
- A resync event after a simulated reconnect gap refreshes stale data.
- `gitPull` does not invalidate `["config"]` or story-detail keys.
- Team lookup still renders configured images and initials fallbacks with one
  observer for the whole board.
- StoryModal chunk is not requested until a card is opened.
- Skeletons expose accessible status text.
- All existing drag, reorder, move, sprint-selection, forecast, and
  phase-expansion tests pass unchanged.

**Acceptance:** B12 met. No unfiltered `invalidateQueries()` remains. Board
startup asset graph excludes StoryModal and DOMPurify. Losing SSE never leaves
a client permanently stale.

---

### WP-06: Static Asset Delivery (`US-034`)

**Model: Sonnet.** The cheapest package to delegate despite touching HTTP
correctness: RFC 9110 validator and range semantics are well-specified, the
acceptance criteria are already enumerated in `US-034`, and every failure mode
(wrong `Content-Range`, missing `Vary`, unsatisfiable range) is directly
assertable. The one judgment call — build-time precompression versus a runtime
`tower-http` `CompressionLayer` — is **already decided in this document**, so
Sonnet executes rather than chooses. Escalate only if adding a `build.rs`
conflicts with the installer or release scripts under `scripts/release`.

**Objective:** Complete the existing draft story `US-034` and stop retransferring
hashed assets.

**Files:** new `crates/web-server/src/static_assets.rs`; `handlers/mod.rs`
(delegation only); `lib.rs` (wiring only); `crates/web-server/build.rs` if
precompressing; `Cargo.toml`.

**Dependency note:** `tower-http` is **not** a workspace dependency and `tower`
is dev-only. Adding `tower-http` with compression features must be an explicit,
reviewed step.

**Compression decision — prefer build time.** Assets are embedded via
`include_dir` and never change at runtime. Precompressing them in `build.rs`
and serving the stored `.br`/`.gz` variant:

- pays compression once at build instead of once per request;
- produces a stable strong `ETag` per representation;
- sidesteps the range-vs-compression interaction entirely, because a range is
  served against the stored representation the client actually negotiated;
- avoids adding `tower-http` and its transitive tree to the binary.

Use a runtime `CompressionLayer` only for dynamic JSON responses, if measurement
after Phase 1 shows JSON transfer is material on loopback. It usually is not.

**Steps:**

1. Move static response construction into `static_assets.rs` first, to minimize
   merge conflict with other packages.
2. Replace `file.contents().to_vec()` (`handlers/mod.rs:502`) with
   `Bytes::from_static` — a zero-risk per-request allocation removal.
3. `Cache-Control: public, max-age=31536000, immutable` for Vite-hashed assets
   under `/assets/`.
4. `Cache-Control: no-cache` for `index.html` and the SPA fallback.
5. Deterministic strong `ETag` from embedded bytes, computed at build time,
   distinct per content-encoding.
6. Honor `If-None-Match` with `304`.
7. `Accept-Ranges: bytes`, `206 Partial Content` with correct `Content-Range`,
   `416` for unsatisfiable ranges — the explicit `US-034` criteria.
8. Content negotiation on `Accept-Encoding` with `Vary: Accept-Encoding`.
9. Preserve MIME detection and SPA fallback exactly.
10. Verify unknown `/api/*` paths return `404`, not `index.html` with `200`.

**Tests:** hashed-asset immutable header; HTML `no-cache`; ETag stability and
`304`; valid range → `206` + `Content-Range`; invalid range → `416`; encoding
selected only when accepted; `Vary` present; MIME and SPA fallback unchanged;
unknown `/api/*` → `404`.

**Acceptance:** B11 met. All `US-034` acceptance criteria satisfied. Lighthouse
no longer reports a zero-second asset cache TTL.

**Boundary:** This *is* `US-034`. Do not create a duplicate story. Update
`US-034`'s stale background reference to `lib.rs:663-684`; the handler now lives
at `handlers/mod.rs:491`.

---

## 11. Phase 3 — Conditional on Gate G1

Build **only** if §9 says so. Both packages are specified at lower detail than
Phase 1 deliberately: their design should be revisited against the G1
measurements rather than pre-committed now.

### WP-07: Page-Specific API Contracts (conditional)

**Model: Opus → Sonnet.** *Opus* owns the two decisions flagged below — the
`DashboardMetrics` breaking-change choice and the sprint-payload strategy — plus
the `typegen` reachability assertion, which is a test that must detect an
*absence* and is therefore easy to write in a way that never fails. *Sonnet*
implements the DTOs, routes, and size assertions once the contract is frozen and
the TypeScript bindings are published. Risk R6.

**Trigger:** G1 shows B5 met but B9/B10 missed, i.e. the residual cost is
payload size, serialization, or client parse — not parsing.

**Contracts:**

```text
GET /api/progress -> ProjectProgress
GET /api/board    -> BoardSnapshot { sprints: BoardSprint[], progress: ProjectProgress }
GET /api/metrics  -> DashboardMetrics { ..., epicProgress: EpicProgressSummary[] }
```

`BoardStorySummary`: `id`, `title`, `status`, `sprint`, `priority`,
`storyPoints`, `assignees`, `taskSummary`. No task descriptions, no
`frontmatter`, no relative paths, no lifecycle timestamps.

`EpicProgressSummary`: `id`, `title`, `phase`, `donePoints`, `totalPoints`,
`doneStories`, `totalStories`.

**Decisions that must be made before any code:**

1. **Extending `DashboardMetrics` is a breaking change** to a shipped endpoint.
   Decide explicitly: extend in place (simpler, breaks any external consumer) or
   add `DashboardResponse` (safer, more churn). Record the choice here before
   WP-05's frontend work is touched.
2. Whether all sprint card summaries fit under B14. If not, return sprint
   metadata plus the active sprint and add `GET /api/board/sprints/{name}`.

**Critical trap:** every new DTO must be registered in `generated_declarations`
(`typegen.rs:51-82`). If it is not, `generated_bindings_are_current`
**passes** while the TypeScript type is silently absent. Add an assertion that
the registered set matches the DTOs actually reachable from a route response.

```sh
cargo test -p kanban-web-server typegen::export_bindings -- --exact            # generate
cargo test -p kanban-web-server typegen::generated_bindings_are_current -- --exact  # verify
```

**Acceptance:** B14, B15, B16 met on the §5.3 fixture; board and dashboard no
longer request `/api/repository`; `/api/repository` still serves backlog and
sprint views; no endpoint performs an extra repository read.

**Handoff:** publish generated TypeScript and example JSON before any frontend
change begins.

### WP-08: Prewarmed Read-Model Cache (conditional)

**Model: Opus, end to end.** The hardest concurrency in the plan and the only
package with no cheap-tier subset. Generation tracking, supersede-during-rebuild,
atomic publication ordering relative to SSE, previous-generation retention on
failure, and eleven deterministic tests under a controlled clock are mutually
entangled — an error in any one produces a cache that serves stale data under
load while passing a naive test suite. If G1 does not require this package, the
correct model choice is "none".

**Trigger:** G1 shows B5 missed **and** profiling attributes the residual cost
to parsing rather than derivation.

**State model:**

```text
ReadModelCache {
  current: RwLock<Arc<WebReadModel>>,   // hold only long enough to clone the Arc
  requested_generation: AtomicU64,
  published_generation: AtomicU64,
  rebuild_tx: bounded mpsc Sender,
}
```

**Key requirements:**

1. Build generation 1 before the readiness line; fail startup loudly if it
   cannot be built.
2. Handlers read a cloned `Arc`; the blocking build never happens under the lock.
3. Reuse WP-04's coalescer as the sole invalidation source. Do not add a second
   debounce.
4. One rebuild per coalesced burst, in `spawn_blocking`; if superseded during a
   build, rebuild once more before publishing.
5. Atomic swap only on success; on failure retain the previous model, log with
   generation and cause, retry on next invalidation.
6. Serve the previous good generation throughout a rebuild.
7. Emit SSE only after the new generation is readable.
8. Pre-serialize response bodies to `Bytes` **only if** measurement shows
   serialization is material. Do not do this speculatively.
9. Add an `AppState` test builder so tests do not construct cache fields by hand.

**Deterministic tests (injected fake builder, controlled Tokio time — no real
sleeps):** ten concurrent misses invoke the builder once; hits do not invoke it;
one burst → one rebuild; an event during a rebuild → at most one follow-up;
requests during rebuild get the previous complete generation; failed rebuild
retains the previous; SSE fires after visibility, not before; successful
mutation and successful `git pull` each invalidate; a failed or no-op mutation
does not publish a false generation; startup is not ready before generation 1.

**Acceptance:** B13 met; a prewarmed endpoint does zero filesystem or Git work;
concurrent board+dashboard share one generation; the cache is memory-only and
always reconstructible from markdown.

**Boundary:** WP-03's builder stays pure. Cache orchestration wraps it; it never
leaks mutable state into parsing.

---

## 12. Backlog Mapping

This document is planning context. The repository's own convention — and the
tool's own purpose — requires the work to live in `delivery/backlog`. Revision 1
existed only as untracked root markdown, repeating the pattern of the deleted
`REFACTORING_PLAN.md`.

Author the following before implementation starts, following the existing
`US-<nnn>-<kebab-slug>.md` format with full frontmatter and Gherkin acceptance
criteria. Current maximum is `US-041`; epics are `EP-001`..`EP-003`.

| Proposed | Title | Maps to | Points |
|---|---|---|---|
| `EP-004` | Web and CLI read-path performance | this document | — |
| `US-042` | Reproducible performance harness and backlog fixture generator | WP-01 | 5 |
| `US-043` | Resolve kanban configuration once per repository read | WP-02 | 8 |
| `US-044` | Build the web read model in a single repository pass | WP-03 | 8 |
| `US-045` | Coalesce filesystem events and give SSE resumable identity | WP-04 | 5 |
| `US-046` | Bounded query staleness, SSE fallback, and deferred modal chunk | WP-05 | 5 |
| `US-034` | Static asset caching and Range headers *(exists, `draft`)* | WP-06 | 3 |

`US-042`..`US-046` link to `EP-004`. `US-034` keeps `EP-003`. Phase 3 stories
are authored only if gate G1 requires them, so the backlog never carries
speculative scope.

Keep this document as the architectural rationale referenced from `EP-004`;
move per-package acceptance criteria into the stories so `kanban validate` and
`kanban doctor` can track them.

---

## 13. Risk Register

| # | Risk | Likelihood | Impact | Mitigation | Owner |
|---|---|---|---|---|---|
| R1 | WP-02 changes parsed output subtly (ordering, containment, path normalization) | Medium | High — silent backlog corruption | Golden-output equivalence test on both fixture configs before/after; `validate` + `doctor` output diff | WP-02 |
| R2 | Removing `find_epic` from `load_epics` changes epic child-story sets or fallback behavior | Medium | High — wrong board data | Snapshot equivalence test including the absent-epic-file fallback case | WP-03 |
| R3 | Clients silently stale after losing SSE | High if `staleTime: Infinity` | High | Bounded 60 s staleness, `onerror` fallback, resumable event ids (WP-04/WP-05) | WP-05 |
| R4 | Debounce ceiling delays visible updates during long `git pull` | Low | Medium | 1 s ceiling emits during sustained bursts; verified in WP-09 step 7 | WP-04 |
| R5 | `tower-http` addition inflates binary size / conflicts with installer expectations | Low | Medium | Prefer build-time precompression; measure binary delta and report in handoff | WP-06 |
| R6 | New DTO omitted from `typegen` registration; TS type silently missing | Medium | Medium | Reachability assertion added in WP-07 | WP-07 |
| R7 | Phase 3 built despite gate G1 passing | Medium | High — ~1,000 lines of unjustified stale-data surface | Gate is a merge blocker; G1 results recorded in this document before any Phase 3 branch | WP-09 |
| R8 | Benchmark fixture inherits `sprints: false` and misses the dominant path | Medium | High — false "already fast" reading | §5.3 pins feature flags; harness asserts the generated config | WP-01 |
| R9 | `api_team_avatar` blocking I/O on the async runtime (`handlers/mod.rs:194, 203`) — residual `US-023` gap | Low | Low | Wrap in `run_blocking`; note as an opportunistic fix in WP-03 | WP-03 |

---

## 14. Deferred Follow-Ups

G1 was recorded in §9.1 and passed, so the two conditional Phase 3 packages move
here:

- **WP-07, page-specific API contracts** (`/api/progress`, `/api/board`, compact
  `EpicProgressSummary`). Revisit only if a repository an order of magnitude
  larger than the §5.3 fixture pushes board or dashboard LCP past B9/B10, and
  only after confirming the residual cost is payload rather than derivation.
- **WP-08, prewarmed read-model cache.** Revisit only if profiling attributes a
  missed B5 to parsing. Note that at 250 stories the entire build is 42 ms, of
  which serialization is 0.6 ms, so the cache would be optimizing a cost that is
  already below human perception.

Other candidates:

- Incremental reparse keyed by changed path and content hash.
- Generation-based `ETag` and `304` on API responses (natural once WP-04
  supplies a monotonic generation counter; cheaper than WP-07 if payload size is
  the residual problem).
- Splitting backlog and sprint views off `/api/repository`.
- Virtualizing very large board columns.
- Replacing or precomputing Recharts geometry.
- Persisting benchmark history as CI artifacts.
- Parallelizing story parsing with rayon — almost certainly unnecessary once
  §3.1 is fixed, since parsing was never the bottleneck.

---

## 15. Cross-Package Handoff Requirements

Each package handoff must state:

1. Exact files changed.
2. Public and internal API changes with migration notes.
3. Tests added and the exact commands run.
4. Before/after benchmark output from the WP-01 harness for its package.
5. Known limitations and deferred items.
6. Confirmation that unrelated worktree changes were untouched.
7. Generated files downstream packages must regenerate or consume.
8. The workspace version bump applied.

Packages changing shared contracts must include example payloads. Packages
changing event or cache semantics must include an event timeline for the
mutation, external-edit, and `git pull` flows.

### 15.1 Recording the model tier

The tier used for a package is recorded **by the orchestrator, from the routing
it performed** — not self-reported by the implementing agent. A subagent cannot
reliably attest to its own model, so a declaration in a handoff is not evidence.

Tier is therefore guaranteed by *which agent was invoked*. The following
subagents are configured in `~/.config/opencode/opencode.json` and map to the
§7.1 tiers:

| Agent | Model | Use for |
|---|---|---|
| `impl-sonnet` | `claude-sonnet-5` | Sonnet-tier packages and the Sonnet half of a `Opus → Sonnet` split, against a frozen brief |
| `runner-haiku` | `claude-haiku-4.5` | The §7.1 command-runner sub-tasks; edit and task tools denied |
| `review-opus` | `claude-opus-5` | Fresh-context review of WP-02, WP-03, WP-04, WP-08 |
| *(orchestrator)* | `claude-opus-5` | Opus-tier packages and all design decisions |

Record per package, in the merge commit or the owning backlog story:

```text
package:   WP-06
routed-to: impl-sonnet (claude-sonnet-5)
brief:     IMPROVEMENT_PLAN.md §10 WP-06, steps 1-10
reviewed:  review-opus, fresh context
escalated: none
```

If a package was routed below its §7 tier, that fact must appear here so the
reviewer can weight scrutiny accordingly. An escalation under §7.1 is recorded
with the trigger that caused it and the agent the work moved to.

---

## 16. Verification (WP-09)

**Model: Sonnet → Opus.** Running the suite, capturing traces, and comparing
against §4.2 is Sonnet work (and the command-running subset is Haiku work per
§7.1). Deciding whether a missed budget warrants a documented exception, and
diagnosing any equivalence failure that surfaces only under integration, is
Opus work.

**Full suite — every package, before handoff:**

```sh
cargo fmt --all -- --check
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cargo build
cargo run -p kanban-cli -- validate .
cargo run -p kanban-cli -- doctor .
npm --prefix web run typecheck
npm --prefix web run test
npm --prefix web run build
cargo test -p kanban-web-server typegen::generated_bindings_are_current -- --exact
```

Note: the `--exact` filter needs the module-qualified name
(`typegen::generated_bindings_are_current`); the bare name matches nothing and
the command exits `0` having run no test.

**Integrated verification — WP-09 only:**

1. WP-01 benchmark against both §5.3 fixture configurations.
2. Benchmark against the IP 2.0 repository when available, reported separately
   as context, never as a gate.
3. Cold Chrome traces (cache disabled) for board and dashboard.
4. Warm/repeat traces (cache enabled) to verify B11.
5. One manual markdown edit → exactly one SSE event, correct refreshed data.
6. Multi-file `git pull` → verified coalescing, no update starvation.
7. Kill and restart the SSE connection mid-session → client resynchronizes and
   shows no stale data.
8. Exceed `SSE_SUBSCRIBER_CAP` → the capped client shows the fallback indicator
   and still refreshes via polling.
9. Exercise story move, reorder, task update, sprint selection, story modal,
   epic detail, dashboard phase expansion, report, and `git pull`.
10. If Phase 3 landed: force a slow rebuild and confirm the previous generation
    stays visible; force a rebuild error and confirm retention plus a useful log.

**Release criteria:**

- All functional, generated-type, and repository validation checks pass.
- Every budget in §4.2 met, or an explicit exception documented and approved
  before release.
- A profile shows no per-story `load_kanban_config` or `git rev-parse` loop.
- A cold board or dashboard produces at most one source read.
- Static assets return the planned cache, validation, range, and encoding
  headers.
- No absolute path or internal error detail is newly exposed by any response.
- Gate G1 results are recorded in §9 of this document.

---

## 17. Observability

Log one structured record per read-model build. Never log per cache hit.

```text
generation
cause: startup | watcher | mutation | git-pull | config-change
story_count, epic_count, sprint_count
config_loads              # must be 1 — the direct regression proxy
git_root_resolutions      # must be 1 — the direct regression proxy
build_duration_ms
serialization_duration_ms
events_coalesced
superseded: bool
result: published | retained_previous | failed
```

`config_loads` and `git_root_resolutions` are deliberately included: they are the
exact quantities that regressed to 6,293, and surfacing them in normal operation
makes a recurrence visible without profiling.

Optional `Server-Timing` in development builds. Never expose repository paths or
internal error detail.

---

## 18. Completion Checklist

**Phase 1 (mandatory)**

- [ ] WP-01 harness reproducible; fixture pins feature flags per §5.3
- [ ] Resolution counter available and asserted (B1-B4)
- [ ] `read_repository` performs exactly one root resolution and one settings parse
- [ ] No config load inside any per-file loop in `crates/core`
- [ ] `kanban validate` and `kanban doctor` output unchanged and within B6
- [ ] `load_epics` no longer calls `find_epic`
- [ ] `api_metrics`, `api_report`, and `load_epic_detail` each read the source once
- [ ] Duplicate `compute_progress` in `metrics.rs:100` removed
- [ ] Wire format byte-identical to pre-refactor for both fixture configs
- [ ] Watcher events coalesced; one burst → one SSE event (B7, B8)
- [ ] SSE events carry monotonic ids; `Last-Event-ID` and lag trigger resync

**Gate**

- [ ] G1 measured, recorded in §9, and the Phase 3 decision documented

**Phase 2 (mandatory)**

- [ ] Bounded `staleTime` + focus refetch + SSE `onerror` fallback in place
- [ ] Unfiltered `invalidateQueries()` after `gitPull` replaced with explicit keys
- [ ] `useTeam` hoisted out of `StoryCard`
- [ ] StoryModal and DOMPurify deferred out of board startup
- [ ] Fixed-dimension skeletons; background refresh keeps content (B12)
- [ ] `US-034` fully satisfied: cache headers, ETag/304, ranges, encoding (B11)
- [ ] `Bytes::from_static` replaces per-request asset copying

**Phase 3 (only if G1 requires)**

- [ ] `DashboardMetrics` breaking-change decision recorded before implementation
- [ ] Board/dashboard/progress payloads within B14/B15/B16
- [ ] `typegen` reachability assertion added
- [ ] Cache deterministic concurrency tests pass; B13 met

**Release**

- [ ] Backlog stories authored per §12 and moved to `done`
- [ ] Full verification suite green
- [ ] Final traces meet B9, B10, B11, B12
- [ ] Workspace version updated per `AGENTS.md`

---

## 19. Implementation Record

Implemented 2026-08-04. Backlog representation: `EP-004` with `US-042`..`US-046`
in `delivery/backlog`; `US-034` completed under its existing `EP-003` ownership.

### 19.1 What landed

| Package | Status | Principal change |
|---|---|---|
| WP-01 | Landed | Generated fixtures (`crates/core/src/testsupport.rs`), thread-local read-path counters (`crates/core/src/instrument.rs`), `scripts/benchmark_web_load.py`, ignored no-HTTP bench (`crates/web-server/src/bench.rs`) |
| WP-02 | Landed | Config-aware collectors/readers threaded through `crates/core`; `find_epic` no longer rescans per call |
| WP-03 | Landed | `crates/web-server/src/read_model.rs`; `find_epic` removed from the epic projection; duplicate `compute_progress` removed |
| WP-04 | Landed | `crates/web-server/src/changes.rs`; debounce-with-ceiling coalescing, monotonic SSE ids, `Last-Event-ID` and lag resync |
| G1 | **Passed** | Recorded in §9.1 |
| WP-05 | Landed | Bounded `staleTime` + SSE `onerror` fallback, scoped invalidation, hoisted team lookup, lazy `StoryModal`, skeletons |
| WP-06 | Landed | `crates/web-server/src/static_assets.rs` + build-time fingerprinting and gzip in `build.rs` |
| WP-07 | **Not built** | Gate G1 passed; moved to §14 |
| WP-08 | **Not built** | Gate G1 passed; moved to §14 |
| WP-09 | Landed | §16 suite green; §9.1 traces captured; version bumped to 26.8.401 |

### 19.2 Budget results

| # | Metric | Target | Result |
|---|---|---:|---|
| B1 | `git rev-parse` spawns per `read_repository` | 1 | **1** — asserted |
| B2 | `git rev-parse` spawns per web read-model build | 1 | **1** — asserted |
| B3 | Complete story parses per read-model build | 1 per story | **1 per story** — asserted |
| B4 | `settings.json` parses per read-model build | 1 | **1** — asserted |
| B5 | Read-model build, 250-story fixture | p95 <= 250 ms | **42.3 ms** |
| B6 | `kanban doctor` on 250-story fixture | <= 500 ms | **51.7 ms** |
| B7 | Rebuilds per filesystem event burst | 1 | **1** — asserted |
| B8 | SSE `change` events per burst | 1 | **1** — asserted, and verified live against a 12-file burst |
| B9 | Board cold LCP | <= 1000 ms | **283 ms** |
| B10 | Dashboard cold LCP | <= 1000 ms | **266 ms** |
| B11 | Repeat-load bytes for unchanged hashed assets | 0 | **0** — `transferSize: 0` for every hashed asset |
| B12 | CLS from loading-state replacement | <= 0.02 | **0.01** on both pages |

B13-B16 do not apply: they were conditional on gate G1 failing.

### 19.3 Deliberate deviations from the plan as written

Each of these is a decision, not an omission.

1. **Phase 3 was not built.** §9 made this the required outcome when B5, B9 and
   B10 are met. They were met with large margins. Recorded in §9.1.
2. **`read_story_file` keeps caller-relative paths.** WP-02 step 2 specified
   `read_story_file_with_config(path, &KanbanConfig)`. Implemented as specified,
   *plus* a preserved `read_story_file(path, repo_root)` that still computes
   relative paths against the caller's `repo_root`. These differ when a caller
   passes `"."`, and CLI call sites (`dispatch.rs`, `doctor.rs`) depend on the
   existing behavior. Silently changing it is exactly the R1 failure mode.
3. **`doctor.rs:140/191/232` were not the hot loop.** The plan described them as
   per-file readers inside a loop. They are actually inside `apply_doctor_fix`'s
   match arms and run once per applied fix. The real `doctor` N+1 was
   `collect_doctor_issues_at_date`: a redundant second full read via
   `validate_repository`, plus `find_epic` per epic file. Both were fixed.
4. **Shared regexes were hoisted to `LazyLock` statics** (`crates/core/src/regexes.rs`).
   Not in the plan. Once the configuration blowup was removed, per-story and
   per-task-file `Regex::new` calls dominated what remained: `doctor` on the
   250-story fixture went from 290 ms to 50 ms and `validate` from 257 ms to
   24 ms purely from this. Immutable statics, no behavior change.
5. **Compression is build-time gzip only; no Brotli, no `tower-http`.** The plan
   preferred build time and left the coding open. gzip via `flate2` as a
   `[build-dependencies]` entry reaches every browser, needs no runtime
   dependency, and cuts the entry chunk from 267,672 to 85,537 bytes. Brotli
   would add a dependency for a marginal further gain on what is usually a
   loopback connection.
6. **The SSE subscriber guard was moved into the stream.** Not in the plan. The
   guard was a local binding in `api_events`, so it dropped when the handler
   returned the response — `SSE_SUBSCRIBER_CAP` counted concurrently executing
   handlers, not concurrently open streams, and therefore bounded nothing.
7. **`api_team_avatar` blocking I/O was wrapped** (risk R9), as the plan
   suggested opportunistically.
8. **Open SSE streams no longer block graceful shutdown.** Not in the plan, and
   found only because the §16 step-7 manual check ("kill the SSE connection
   mid-session") did not behave as expected. `axum::serve(..)
   .with_graceful_shutdown(..)` waits for in-flight connections after the signal
   resolves, and an SSE stream never finishes, so `kanban web serve` survived
   SIGTERM for as long as any browser tab was open and `kanban web stop`
   (`web.rs:563`) always spent its full 3 s window before falling through to
   SIGKILL. A `watch<bool>` set *before* the shutdown future resolves now ends
   the streams. Measured: exit 0.02 s after SIGTERM with a stream open.

### 19.4 Public API surface added to `kanban-core`

No public API was removed. Added:

```text
config:      (none — load_kanban_config unchanged)
repository:  collect_user_story_files_with_config, collect_epic_files_with_config,
             read_story_file_with_config, read_epic_file_with_config,
             read_repository_with_config
epic:        find_epic_in_repository, read_epic_sources, select_epic_source,
             epic_details_from_source
sprint:      summarize_sprints_from_repository (was pub(crate), now takes &KanbanConfig)
story:       story_overviews_from_repository
validate:    validate_parsed_repository
feature `test-support`: testsupport::{FixtureSpec, BacklogFixture, generate_backlog_fixture},
             instrument::{ReadPathCounters, ReadPathCounts}
```

### 19.5 Reproducing the measurements

```sh
# Deterministic guards (part of the normal suite)
cargo test

# No-HTTP timings: read-model build, doctor, validate
cargo test -p kanban-web-server --release -- --ignored --nocapture read_path_bench

# HTTP timings against a generated fixture
cargo test -p kanban-web-server --release -- --ignored --nocapture materialize_fixture
./target/release/kanban web serve --repo-root <printed path> --host 127.0.0.1 --port 3999
python3 scripts/benchmark_web_load.py --base-url http://127.0.0.1:3999 --runs 20
```

Browser figures follow §WP-01 step 8: fresh context, `about:blank`, start
recording, navigate once.

### 19.6 Known limitations and residual work

- `story_overview` still calls `epic_title`, which performs one `read_dir` plus
  one file read per story to resolve an epic title from the story's own
  directory. At 250 stories this is a few milliseconds inside a 42 ms build, so
  it was left alone rather than memoized. If a repository an order of magnitude
  larger appears, a `(directory, epic_id)` memo is the obvious next step and is
  semantics-preserving.
- The `/api/repository` payload is 947 KB uncompressed on the 250-story fixture.
  Irrelevant on loopback; over a network deployment (`Dockerfile`,
  `docker-compose.yml`) a runtime `CompressionLayer` on JSON responses, or the
  generation-based `ETag` in §14, would be the cheaper first move — cheaper than
  the payload splitting of WP-07.
- The SSE subscriber-cap fallback is covered by unit tests
  (`sse_subscriber_cap_rejects_over_limit`, and the client `onerror` test), not
  by an end-to-end 65-client scenario.

### 19.7 Manual verification performed

| Check | Result |
|---|---|
| Board, backlog, sprints, dashboard, report all render; no console errors | Pass |
| Story modal opens; its chunk and `marked`/DOMPurify are requested only on first open | Pass |
| Sprint selection switches the board | Pass |
| Epic detail (`/api/epics/EP-001`) returns the epic, 8 child stories and a body | Pass |
| 12-file edit burst produces exactly one SSE `change` with a monotonic id | Pass |
| Reconnect with a stale `Last-Event-ID` receives an immediate `resync`; a current one receives nothing | Pass |
| Killing the server surfaces "Live updates unavailable" and keeps the last board on screen | Pass |
| Restarting the server withdraws the indicator | Pass |
| SIGTERM with an open SSE stream exits in 0.02 s | Pass |
| Repeat load transfers 0 bytes for every hashed asset | Pass |
