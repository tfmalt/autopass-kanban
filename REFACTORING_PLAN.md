# Refactoring Plan: Deduplication & Architecture Cleanup

Status: proposed · Date: 2026-07-02 · Source: codebase analysis session

Goal: one source of truth for every domain rule, one DTO conversion layer,
one CLI dispatch tree, and mechanical boilerplate collapsed — without changing
observable behavior (CLI output, JSON schema, HTTP API, or backlog markdown).

## How to use this plan

Each task below is self-contained and written to be delegated to a separate
agent. Tasks state their **complexity** (Low / Medium / Hard), **dependencies**,
and whether they are **parallel-safe** (touch disjoint files from other tasks).

Suggested model mapping:

| Complexity | Model | Rationale |
|---|---|---|
| Low | Haiku 4.5 | Mechanical, well-specified, low judgment |
| Medium | Sonnet 5 | Localized design decisions, moderate blast radius |
| Hard | Opus 4.8 / Fable 5 | Cross-cutting refactors, API design, behavior-preservation risk |

### Conventions binding on every task

1. Follow `AGENTS.md`. In particular: backlog semantics stay in `crates/core`;
   markdown is the source of truth; no silent rewrites of unrelated content.
2. Verification (run from repo root, all must pass before a task is done):
   - `cargo fmt --all -- --check`
   - `cargo test`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo build`
   - For tasks touching markdown parse/write behavior:
     `cargo run -p kanban-cli -- validate .` and `cargo run -p kanban-cli -- doctor .`
   - For tasks touching `web/`: `npm --prefix web run typecheck`,
     `npm --prefix web run test`, `npm --prefix web run build`
3. Bump the workspace version in `Cargo.toml` per the AGENTS.md SemVer scheme
   when finishing a task.
4. **Behavior freeze:** unless the task explicitly says otherwise, JSON output
   (`--format json`), HTTP responses, and rendered CLI output must be
   byte-identical. Add snapshot/characterization tests first if coverage is thin.
5. Do not start two tasks that list overlapping files at the same time.

---

## Phase 0 — Independent quick wins (all parallel-safe with each other)

### T01 · Unify assignee parsing in Rust — **Low**

- **Problem:** `crates/core/src/util.rs::parse_assignee_list` and
  `crates/web-server/src/team.rs::parse_assignees` are near-duplicates with
  divergent behavior: the web-server copy also filters the literal placeholder
  `Name <email@example.com>`; core's does not.
- **Change:** Add the placeholder filter to `parse_assignee_list`, expose it
  `pub` from core (re-export via `lib.rs`), delete
  `team.rs::parse_assignees`, update `snapshot.rs` to call the core function.
- **Files:** `crates/core/src/util.rs`, `crates/core/src/lib.rs`,
  `crates/web-server/src/team.rs`, `crates/web-server/src/snapshot.rs`.
- **Accept:** one Rust implementation; unit tests cover `~`, `TBD`,
  placeholder email, comma lists; all existing tests pass.

### T02 · Collapse HTTP verb helpers in web client — **Low**

- **Problem:** `web/src/api/client.ts` `postJson`/`putJson`/`patchJson` are
  identical except the method string.
- **Change:** one `sendJson(method, url, body)` helper; keep the exported
  API functions unchanged.
- **Files:** `web/src/api/client.ts` (+ `client.test.ts`).
- **Accept:** no exported signature changes; web tests pass.

### T03 · Extract sprint roster markdown rendering from core/sprint.rs — **Low**

- **Problem:** `crates/core/src/sprint.rs` (1,249 lines) mixes sprint domain
  operations with markdown roster rendering (`render_sprint_roster_section`,
  `render_sprint_roster_summary`, `escape_markdown_*`, `escape_table_cell`,
  `push_line`, `sprint_story_link_label`, `render_assignee_cell`,
  `format_task_summary`, `status_summary_label`). `relative_path_from`
  (line ~776) re-implements `util::relative_path`.
- **Change:** move rendering helpers to a new `crates/core/src/sprint_roster.rs`
  (crate-private), keep public API on `sprint.rs` unchanged. Replace
  `relative_path_from` with the existing util where semantics match; if they
  differ (it computes `..`-style relative paths), keep it but move it to
  `util.rs` with a doc comment distinguishing the two.
- **Files:** `crates/core/src/sprint.rs`, new `sprint_roster.rs`,
  `crates/core/src/util.rs`, `crates/core/src/lib.rs`.
- **Accept:** pure move; generated sprint README markdown byte-identical
  (existing roster tests must not change); `validate`/`doctor` clean.

### T04 · Move shell-completion script text out of Rust strings — **Low**

- **Problem:** `crates/cli/src/completion.rs` (1,564 lines) embeds ~1,400
  lines of zsh/bash script as string constants.
- **Change:** move the script bodies to
  `crates/cli/src/completion/dynamic.zsh` and `dynamic.bash`, pull in with
  `include_str!`. No content changes.
- **Files:** `crates/cli/src/completion.rs` + new script files.
- **Accept:** `crates/cli/tests/completion.rs` (1,312 lines of tests) passes
  unmodified — that suite is the byte-level safety net.

### T05 · Extract process supervision from cli/web.rs — **Low**

- **Problem:** `crates/cli/src/web.rs` (1,188 lines) mixes per-OS process
  supervision (pid files, signals, zombie detection, `process_exists`,
  `terminate_process`, `force_kill_process`, cfg-gated unix/windows variants)
  with port resolution, DTOs, and printing.
- **Change:** mechanical move of the process-lifecycle functions into
  `crates/cli/src/web/process.rs` (or `web_process.rs`), keeping `web.rs` as
  orchestration. No logic changes.
- **Files:** `crates/cli/src/web.rs`, new module file, `crates/cli/src/main.rs`
  (module decl).
- **Accept:** `kanban web start/stop/status/restart/log` behave identically;
  cargo test passes on the workspace.

---

## Phase 1 — Typed domain core (the load-bearing workstream)

> Rationale: the "dropped stories" bug was fixed three times in three layers
> (commits `7266e95`, `675a1e9`, `87d7544`) because status semantics are
> re-encoded as string comparisons in ≥7 places. These tasks create the single
> source of truth.

### T10 · Introduce `StoryStatus` enum in core — **Hard**

- **Problem:** status is a raw `String` everywhere. `done | dropped` matches
  appear at `story.rs:96,192,235,651`, `json.rs:1412`, `validate.rs:186,558`;
  `web-server` and the frontend re-derive the same rules.
- **Change:** in `crates/core` (new `status.rs`):
  ```rust
  pub enum StoryStatus { Draft, Backlog, Ready, Planned, Todo,
      InProgress, ReadyForQa, Done, Blocked, Dropped }
  ```
  with `parse` (absorbing `util::normalize_status_alias` aliases: `backlog`→
  `ready` mapping stays in normalize, not in the enum), `as_str()`,
  `Display`, and semantic methods:
  - `is_terminal()` → `Done | Dropped`
  - `counts_toward_scope()` → `!Dropped`
  - `board_bucket()` → `Dropped` maps to `Done`, else self
  - `rank()` (replaces `constants::status_rank` / `STATUS_PROGRESSION`)
  Replace string matches inside **core only** (story.rs, validate.rs,
  json.rs, constants.rs consumers). Keep frontmatter reads/writes as strings
  at the file boundary — parse once, format on write.
- **Constraint:** unknown/legacy status strings currently pass through
  unchanged (`normalize_status_alias` doc). Preserve that: model as
  `StoryStatus::parse -> Option<StoryStatus>` or a `Status::Other(String)`
  variant — the implementer chooses, but human-edited files with odd statuses
  must not start failing. `validate`/`doctor` behavior must not change.
- **Files:** new `crates/core/src/status.rs`; `constants.rs`, `util.rs`,
  `story.rs`, `validate.rs`, `json.rs`, `sprint.rs`, `lib.rs`.
- **Accept:** zero `"done" | "dropped"` string matches left in core;
  `CANONICAL_STORY_STATUSES` derived from the enum; all CLI JSON output
  byte-identical (run `cargo test` incl. `json_output.rs` suite);
  `validate .` and `doctor .` output unchanged on this repo.

### T11 · Typed `StoryFields` parsed view on core `Story` — **Hard**

- **Depends on:** T10.
- **Problem:** `Story.frontmatter: BTreeMap<String, String>` (model.rs:45)
  forces every consumer to re-parse `status`, `story_points`, `priority`,
  `assignee`, and lifecycle dates: `json.rs` DTO builders,
  `web-server/snapshot.rs::web_story_from_core` (frontmatter.get chains),
  `validate.rs`, `doctor.rs`.
- **Change:** add a typed accessor layer on `Story` (either an embedded
  `fields: StoryFields` populated at parse time in `repository.rs`, or lazy
  accessor methods — prefer eager struct so errors surface at read time):
  id, status (`StoryStatus`), epic, sprint, priority, story_points,
  assignee raw + parsed list, work_started/work_done/activated/created/updated.
  Keep the raw `frontmatter` map — writers and `frontmatter_keys` ordering
  logic still need it. Migrate core consumers to the typed fields.
- **Files:** `crates/core/src/model.rs`, `repository.rs`, `story.rs`,
  `json.rs`, `validate.rs`, `doctor.rs`.
- **Accept:** `snapshot.rs`-style `frontmatter.get("...")` chains are gone
  from core read paths; write paths unchanged (no markdown diffs on
  `validate`/`doctor`/roundtrip tests); JSON output byte-identical.

### T12 · Web-server consumes core status/scope semantics — **Medium**

- **Depends on:** T10 (T11 helpful but not required).
- **Problem:** `metrics.rs:11-13` and `snapshot.rs:11-21` each define
  `counts_toward_scope` / `board_bucket_status`; `summarize_web_tasks`
  re-implements task counting; `snapshot.rs::compute_progress` encodes
  done/dropped rules locally.
- **Change:** delete both local rule functions, call
  `StoryStatus::counts_toward_scope()` / `board_bucket()`. Reconcile
  `WebTaskSummary` (has `ready_for_qa`) with core `TaskSummary` (doesn't):
  extend core's `TaskSummary` counting to a shared function that both use,
  keeping each JSON shape unchanged.
- **Files:** `crates/web-server/src/metrics.rs`, `snapshot.rs`, `dto.rs`,
  `crates/core/src/model.rs` (task summary helper).
- **Accept:** `/api/repository` and `/api/metrics` responses unchanged
  (metrics tests in `metrics/tests.rs` pass unmodified); no
  `"dropped"` string literals left in `crates/web-server`.

---

## Phase 2 — One DTO layer, generated frontend types

### T20 · Generate `web/shared/types.ts` from Rust DTOs — **Medium**

- **Problem:** `web/shared/types.ts` hand-mirrors `web-server/src/dto.rs`
  (Story/Sprint/Epic/Progress/Metrics/Config/GitPull shapes) plus status
  constant arrays mirroring `core/constants.rs`. A field rename in `dto.rs`
  breaks the frontend silently at runtime.
- **Change:** derive `ts_rs::TS` (feature-gated or dev-dependency) on the
  `dto.rs` and `metrics.rs` response types; add a generator (test-based
  `#[test] fn export_bindings()` or `cargo run -p kanban-web-server --bin
  export-types`) writing `web/shared/generated/api.ts`. Split current
  `types.ts`: generated shapes come from the new file; hand-written helpers
  (`parseAssignees`, `abbreviateAssignee`, `normalizeStatus`, status arrays)
  move to `web/shared/domain.ts` with status arrays exported from Rust
  constants too if ts-rs supports it, else generated into the same file.
  Add npm script + CI/`cargo test` check that regenerating produces no diff.
- **Files:** `crates/web-server/src/dto.rs`, `metrics.rs`, `Cargo.toml`;
  `web/shared/*`; imports across `web/src/**` (mechanical).
- **Accept:** `git diff --exit-code web/shared/generated` after regeneration;
  web typecheck/test/build pass; no hand-maintained interface duplicates a
  serialized Rust type.

### T21 · Snapshot builders map from core types, not raw frontmatter — **Hard**

- **Depends on:** T11.
- **Problem:** `snapshot.rs` re-implements core parsing:
  `web_story_from_core` reads the raw frontmatter map, plus local
  `phase_from_id`, `title_from_body`, `empty_to_none`, `parse_i64`,
  `parse_non_negative_i64`, `extract_section` — most have core equivalents
  or belong in core.
- **Change:** build `WebStory`/`WebEpic`/`WebSprint` from core's typed
  `StoryFields` / `StoryOverview` / `EpicDetails` / `SprintOverview`. Move
  genuinely shared helpers (`title_from_body`, `phase_from_id`,
  `extract_section`) into core (they encode backlog conventions — AGENTS.md
  says backlog semantics live in core). Keep the `Web*` DTO shapes and JSON
  output identical.
- **Files:** `crates/web-server/src/snapshot.rs`, `crates/core/src/*`
  (helper homes), `crates/web-server/src/dto.rs`.
- **Accept:** `snapshot.rs` contains no frontmatter string parsing; HTTP
  response snapshots unchanged; core gains unit tests for the moved helpers.

---

## Phase 3 — core/json.rs decomposition & forecast unification

### T30 · Split `core/json.rs` into envelope / DTOs / domain reports — **Medium**

- **Problem:** `json.rs` is 2,317 lines mixing three concerns: the JSON
  envelope machinery, CLI DTO conversions, and *real domain logic* — the
  Monte Carlo forecast (`ForecastInputs`, percentile bands, ~lines
  1080–1500) and WBS report building.
- **Change:** pure file reorganization inside core, public API preserved via
  re-exports in `lib.rs`:
  - `envelope.rs` — `JsonEnvelope`, `ResultStatus`, `KanbanErrorCode`, `KanbanErrorBody`
  - `dto.rs` (or `json/dto.rs`) — all `*Dto` structs + `from_result` impls
  - `forecast.rs` — `ForecastInputs` + computation, operating on core domain types
  - `report.rs` — WBS building
  Move tests alongside their code.
- **Files:** `crates/core/src/json.rs` → four modules; `lib.rs`.
- **Accept:** no public-path breakage for `kanban-cli`/`kanban-web-server`
  (re-exports keep `kanban_core::X` stable); JSON output byte-identical;
  file sizes: no core module > ~800 lines afterward.

### T31 · Remove the metrics round-trip conversion — **Medium**

- **Depends on:** T30 (and T21 ideally).
- **Problem:** `web-server/metrics.rs::build_forecast` converts `WebStory` →
  core `StoryOverview` (`story_overview_from_web`,
  `sprint_overview_from_web`) to call core's forecast, then converts core's
  `ReportForecastDto` back into its own `Forecast` via `From`. That is
  core → Web → core → Web.
- **Change:** compute metrics from the core repository snapshot directly
  (core types in, `DashboardMetrics` DTO out). Delete both `*_from_web`
  converters and the `From<ReportForecastDto>` impl if the DTO can be built
  in one hop.
- **Files:** `crates/web-server/src/metrics.rs`, `snapshot.rs`, `lib.rs`.
- **Accept:** `/api/metrics` unchanged (existing tests); no Web→core
  conversion functions remain.

### T32 · Finish typed-error migration, delete `classify` heuristic — **Medium**

- **Problem:** `KanbanErrorCode::classify` (json.rs:52-77) derives error
  codes by substring-matching error prose — brittle; the typed
  `KanbanError` path (US-025) already exists as the preferred branch in
  `KanbanErrorBody::from_anyhow`.
- **Change:** audit `bail!`/`anyhow!` sites in core that surface to the CLI
  JSON path; convert the ones that reach envelopes to `KanbanError`
  variants; then delete `classify` and the legacy fallback (fallback becomes
  `Internal`).
- **Files:** `crates/core/src/error.rs`, `json.rs`/`envelope.rs`, core
  modules with user-facing `bail!`s; `crates/cli/tests/json_output.rs`.
- **Accept:** `json_output.rs` error-case tests still see the same `code`
  values; `classify` deleted; grep shows no message-substring classification.

---

## Phase 4 — CLI single dispatch

### T40 · Replace glob imports with explicit imports in kanban-cli — **Low**

- **Problem:** every CLI module opens with
  `#[allow(unused_imports)] use crate::{cli::*, completion::*, ...}::*;`,
  hiding symbol provenance and suppressing unused-import warnings crate-wide.
- **Change:** mechanical replacement with explicit imports; drop the
  `#[allow(unused_imports)]` attributes. No logic changes.
- **Files:** all of `crates/cli/src/**`.
- **Scheduling note:** giant mechanical diff — do **not** run concurrently
  with T41/T05/T04; land it either before T41 starts or after it merges.
- **Accept:** clippy `-D warnings` passes without the allow attributes.

### T41 · Single command dispatch: execute once, render twice — **Hard**

- **Depends on:** T40 landed (merge-conflict avoidance), T30 (envelope module).
- **Problem:** three full matches over the `Command` enum:
  `main.rs::run()` (human path, ~1,000 lines), `json_out.rs::emit_json()`
  (JSON path), and `json_out.rs::command_json_kind()`. Every command's
  argument unpacking and core invocation is written twice; feature-gate
  checks exist in parallel variants (`ensure_*_enabled_json` vs human).
- **Change:** introduce a `CommandOutcome` enum (one variant per command,
  wrapping the existing core result types — `MoveStoryResult`,
  `RolloverResult`, `SprintOverview`, etc.). One `execute(command) ->
  Result<CommandOutcome>` function owns argument handling, feature gates,
  and core calls. Two renderers consume it: `render_human(&theme, &outcome)`
  (absorbs main.rs print logic, delegating to the existing `render/`
  modules) and `render_json(&outcome) -> JsonEnvelope` (absorbs
  emit_json + command_json_kind — `kind` becomes a method on the outcome).
  Migrate incrementally command-group by command-group (config → sprint →
  story → task → …), keeping both old paths compiling until the last group
  moves.
- **Files:** `crates/cli/src/main.rs`, `json_out.rs`, new `dispatch.rs` /
  `outcome.rs`, `render/*`.
- **Accept:** `cli/tests/json_output.rs` (1,808 lines) passes unmodified —
  this is the contract test for the JSON path; human-output tests/goldens
  unchanged; exactly one `match Command` for execution; `emit_json` and
  `command_json_kind` deleted.
- **Delegation note:** this is the largest single task. It can itself be
  split per command group after the `CommandOutcome` skeleton lands (the
  skeleton is Hard; each per-group migration is Medium and parallel-safe
  across groups if they avoid touching shared files simultaneously).

---

## Phase 5 — Frontend cleanup

### T50 · Optimistic-mutation helper for repository snapshot — **Medium**

- **Problem:** in `web/src/api/hooks.ts`, five mutations (`useMoveStory`,
  `usePlanStory`, `useUnplanStory`, `useReorderStories`, `useReorderEpics`,
  `useUpdateSprint`) repeat the identical optimistic skeleton
  (`cancelQueries` → snapshot `previous` → rollback in `onError` →
  invalidate in `onSettled`) and each hand-writes the same traversal
  updating a story in `stories`, `epics[].stories`, and
  `sprints[].storiesByStatus`.
- **Change:** extract into `web/src/api/optimistic.ts`:
  - `useOptimisticSnapshotMutation({ mutationFn, apply })` wrapping the
    skeleton (generic over vars; `apply(snapshot, vars) => snapshot`).
  - Pure snapshot helpers with unit tests:
    `patchStoryEverywhere(snapshot, id, patch)`,
    `removeStoryFromSprints(snapshot, id)`,
    `moveStoryToBucket(snapshot, sprintName, status, story)`.
  Rewrite the five mutations on top; `useUpdateStoryFields`' cache-patch
  logic can stay bespoke.
- **Files:** `web/src/api/hooks.ts`, new `optimistic.ts`,
  `web/src/api/hooks.test.tsx`.
- **Accept:** hooks.test.tsx passes (extend for the new helpers);
  hooks.ts shrinks by ≥150 lines; no behavior change in board/backlog DnD
  (manually verify move/plan/unplan/reorder against the dev server).

### T51 · Extract story-card components from BacklogView — **Medium**

- **Problem:** `web/src/views/BacklogView.tsx` (686 lines) defines five
  card variants (`SortableBacklogStoryCard`, `DraggableBacklogStoryCard`,
  `BacklogStoryCard`, `SprintStoryCard`, `BacklogStoryCardBody`) plus
  epic-preview parsing, alongside the separate
  `web/src/components/StoryCard.tsx`.
- **Change:** move card components to `web/src/components/backlog/`
  with one shared card body and a thin sortable/draggable wrapper
  (dnd-kit `useSortable`/`useDraggable` differ only in transition handling);
  move `parseEpicPreview`/`EpicContext` into components. View file keeps
  layout + DnD orchestration only.
- **Files:** `web/src/views/BacklogView.tsx`, new component files,
  `BacklogView.test.tsx`.
- **Accept:** BacklogView.tsx ≤ ~300 lines; existing view tests pass;
  drag/drop behavior verified in the running app.

### T52 · Single client-side slugify + status helpers module — **Low**

- **Depends on:** T20 (lands `web/shared/domain.ts`).
- **Problem:** `slugifyHeadline` in `hooks.ts:349` duplicates
  `core/util.rs::slugify_headline` char-for-char (needed client-side for
  optimistic sprint rename — cannot be deleted, but must be pinned to the
  Rust behavior).
- **Change:** move to `web/shared/domain.ts`; add a parity test whose cases
  mirror the Rust unit tests (same inputs/outputs, referenced by comment in
  both test files so they get updated together).
- **Files:** `web/src/api/hooks.ts`, `web/shared/domain.ts` + tests.
- **Accept:** one TS implementation; parity test present in both languages.

---

## Phase 6 — Reporting consolidation (largest, do last)

### T60 · Server-computed report rows for the frontend — **Hard**

- **Depends on:** T30, T31, ideally T20.
- **Problem:** throughput/estimate/WBS logic exists three times:
  core (`forecast.rs`/`report.rs` after T30), the frontend
  (`web/src/report/estimates.ts`, `sprints.ts`, `wbs.ts` — computing
  estimates, hours-per-point, sprint projections client-side), and Python
  (`scripts/wbs_report.py`, 924 lines, re-deriving hierarchy + estimates
  from CLI JSON).
- **Change:** extend core's report module to produce the derived rows the
  frontend currently computes (estimates per story, hours/point, sprint
  projections, phase rollups); expose via a new `/api/report` endpoint
  (and keep `/api/metrics` as-is). Rewrite `web/src/report/*` to render
  precomputed rows; keep only presentation concerns (`meta.ts` labels)
  client-side.
- **Files:** `crates/core/src/report.rs`, `crates/web-server/src/handlers`,
  `dto.rs`, `web/src/report/*`, `web/src/views/ReportView.tsx` + tests.
- **Accept:** `web/src/report/estimates.ts` and `sprints.ts` contain no
  arithmetic beyond formatting; ReportView renders identical numbers for the
  fixture dataset (`web/src/report/fixtures.ts`) — write a comparison test
  before switching.

### T61 · Python WBS script consumes precomputed rows — **Medium**

- **Depends on:** T60 (core emits the derived rows in `report wbs` JSON).
- **Problem:** `scripts/wbs_report.py` re-derives WBS numbering, estimates,
  and prognosis that core/frontend also derive.
- **Change:** extend the `kanban --format json report wbs` DTO with the
  derived fields (additive — schema_version bump per policy in `json.rs` if
  shapes change); strip the derivation logic from the Python script, leaving
  xlsx styling/layout. Update `scripts/test_wbs_report.py`.
- **Files:** `crates/core/src/report.rs`, `scripts/wbs_report.py`,
  `scripts/test_wbs_report.py`.
- **Accept:** generated xlsx equivalent for the current backlog (same cell
  values); python tests pass; derivation code deleted from the script.

---

## Dependency graph & suggested waves

```
Wave 1 (parallel): T01 T02 T03 T04 T05 T40
Wave 2:            T10 ──► T11 ──► T21
                   T10 ──► T12
                   T20 (independent of T10; coordinate types.ts with T52)
                   T30 (after T10 lands to avoid json.rs conflicts)
Wave 3:            T31 (after T30, T21)   T32 (after T30)
                   T41 (after T40, T30)   T50 T51 (parallel, web-only)
                   T52 (after T20)
Wave 4:            T60 ──► T61
```

File-conflict warnings:
- `crates/core/src/json.rs`: touched by T10, T11, T30, T32 — serialize these.
- `crates/cli/src/**`: T40 vs T41/T04/T05 — land T40 first.
- `web/shared/types.ts`: T20 vs T52 — T20 first.
- `crates/web-server/src/snapshot.rs`: T01, T12, T21, T31 — serialize.

## Effort summary

| Task | Title | Complexity |
|---|---|---|
| T01 | Unify assignee parsing (Rust) | Low |
| T02 | Collapse HTTP verb helpers | Low |
| T03 | Extract sprint roster rendering | Low |
| T04 | Completion scripts via include_str! | Low |
| T05 | Extract CLI process supervision | Low |
| T40 | Explicit imports in CLI | Low |
| T52 | Shared client-side domain helpers | Low |
| T12 | Web-server uses core status semantics | Medium |
| T20 | Generate TS types from Rust | Medium |
| T30 | Split core/json.rs | Medium |
| T31 | Remove metrics round-trip | Medium |
| T32 | Delete error-classify heuristic | Medium |
| T50 | Optimistic-mutation helper | Medium |
| T51 | Extract backlog card components | Medium |
| T61 | Python WBS consumes precomputed rows | Medium |
| T10 | StoryStatus enum in core | Hard |
| T11 | Typed StoryFields on Story | Hard |
| T21 | Snapshot builders from core types | Hard |
| T41 | CLI single dispatch | Hard |
| T60 | Server-computed report rows | Hard |
