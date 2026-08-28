---
id: US-044
type: user-story
status: done
epic: EP-004
sprint: S001.rolling-thunder
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 8
priority: 30
work_started: 2026-08-04T10:00:13+0200
work_done: 2026-08-04T10:00:13+0200
created: 2026-08-04T10:00:13+0200
updated: 2026-08-27T10:06:07+0200
activated: 2026-08-27T10:06:07+0200
---

# User Story: Build the web read model in a single repository pass

---

## Story Statement

**As a** user of the kanban web board and dashboard,
**I want** every served projection derived from one repository read,
**so that** a page loads in milliseconds and the data it shows is internally
consistent.

---

## Background

`load_epics` read each epic file, computed its overview, and then
called `find_epic` for the same epic — a full `read_repository` plus a full
epic-file rescan, once per epic. The `source_overview` it had already computed
was used only for its `.id`; every other field came back from the second read.

With 214 stories and 27 epics that made `/api/repository` cost about 6,293
configuration loads, of which `load_epics` was 93%. `/api/metrics` then called
`list_all_stories` and `summarize_sprints` for two more full reads, `/api/report`
repeated the pattern, and `/api/epics/{id}` was worse than all of them.
`compute_metrics` also recomputed `compute_progress` even though the snapshot
already carried it, so the two could drift.

---

## Acceptance Criteria

**Scenario 1: One build reads the source once**

```gherkin
Given a generated backlog fixture
When the web read model is built
Then exactly one git root resolution and one settings parse occur
And each story is parsed once and each epic file is parsed once
And deriving the snapshot, metrics, report and epic detail performs no further
    filesystem or git work
```

**Scenario 2: The epic projection is unchanged**

```gherkin
Given a generated backlog fixture in either feature configuration
When the epic projection is built without `find_epic`
Then it is byte-identical to the projection the `find_epic`-based algorithm
    produced, including epic ordering, metadata and child-story lists
```

**Scenario 3: A story whose epic file is missing keeps its fallback**

```gherkin
Given a story whose `epic` names an epic with no epic file
When the snapshot is built
Then a fallback epic is synthesized from the id
And the story is grouped under it
And a story with no epic at all is not grouped
```

**Scenario 4: Metrics and report are unchanged**

```gherkin
Given a generated backlog fixture
When metrics and the report are derived from the single read model
Then burndown, burnup, lead time, velocity, forecast, progress and the full
    report payload are identical to deriving them from separately loaded inputs
```

**Scenario 5: Progress has one source**

```gherkin
Given a built read model
Then the dashboard's `progress` is the same value the repository snapshot serves
```

---

## Non-Functional Requirements

| Area | Requirement |
| ---- | ----------- |
| **Performance** | Read-model build p95 <= 250 ms on the 250-story fixture |
| **Correctness** | Repository, progress, metrics and report derive from the same source read |
| **Backward compatibility** | The HTTP wire format is unchanged; this is observable only as latency |

---

## Technical Notes

- **Requirement refs:** `EP-004#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/read_model.rs` (new),
  `snapshot.rs`, `metrics.rs`, `handlers/mod.rs`
- **Duplicate-id semantics preserved:** `find_epic` always returned the
  lowest-`relative_path` epic for a given id, case-insensitively, and skipped
  epic files with no `id` frontmatter. `build_epics` reproduces both by resolving
  each source through `select_epic_source` before building its `WebEpic`, rather
  than keying naively on each file's own id.
- **Equivalence testing:** the test module contains a reference implementation of
  the pre-change algorithm and asserts serialized equality against it, with
  `generatedAt` stripped because it is `Local::now()`.
- **Opportunistic fix:** `api_team_avatar` moved its `canonicalize` and `read`
  into `run_blocking`, closing a residual `US-023` gap.

### Estimation Rules

`story_points` is `8` (complexity: high).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [x] `find_epic` is not called from the epic projection
- [x] No handler calls `list_all_stories` or `summarize_sprints` after the source is loaded
- [x] The duplicate `compute_progress` in `metrics.rs` is removed
- [x] Golden-output equivalence for snapshot, metrics, report and epic detail
- [x] Wire format byte-identical for both fixture configurations
- [x] Full verification suite passes

---

## Dependencies

| Dependency | Type | Status | Notes |
| ---------- | ---- | ------ | ----- |
| US-043 | Story | Done | Supplies the config-aware core APIs this story composes |
| US-023 | Story | Done | Blocking work must stay inside `spawn_blocking` |

---

## Notes and Open Questions

| #   | Question / Assumption | Owner | Due | Resolved |
| --- | --------------------- | ----- | --- | -------- |
| None | - | - | - | - |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
