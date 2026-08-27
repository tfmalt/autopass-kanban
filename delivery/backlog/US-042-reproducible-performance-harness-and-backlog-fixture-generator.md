---
id: US-042
type: user-story
status: done
epic: EP-004
sprint: S001.rolling-thunder
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
priority: 10
work_started: 2026-08-04T10:00:13+0200
work_done: 2026-08-04T10:00:13+0200
created: 2026-08-04T10:00:13+0200
updated: 2026-08-27T10:06:07+0200
activated: 2026-08-27T10:06:07+0200
---

# User Story: Reproducible performance harness and backlog fixture generator

---

## Story Statement

**As a** maintainer of the kanban read path,
**I want** generated backlog fixtures, read-path instrumentation counters, and a
benchmark harness,
**so that** every performance claim is reproducible without a DevTools session,
one developer's machine state, or an external repository checkout.

---

## Background

The profiling that motivated `EP-004` was done against an external
AutoPASS IP 2.0 checkout. That is measurement context only: no test may depend
on it, and `AGENTS.md` forbids wall-clock assertions in the normal suite.

Wall-clock timings are also the wrong regression guard. The defect being fixed
is *how many times* an expensive operation happens, not how long it takes, so
the durable guard is a count.

A fixture that inherits this repository's defaults would set
`features.sprints = false` and therefore skip the sprint derivation entirely,
under-measuring the very code path that carried most of the cost. Feature flags
must be pinned explicitly.

---

## Acceptance Criteria

**Scenario 1: A fixture exercises the dominant code path**

```gherkin
Given a generated representative fixture
Then it has 250 stories, 30 epics, 5 sprints and ~180 sibling task files
And its configuration pins `features` to `{ phases: false, sprints: true, epics: true }`
And it is created in a temporary directory that has been `git init`-ed
```

**Scenario 2: Edge cases are represented**

```gherkin
Given a generated representative fixture
Then at least one story has no epic
And at least one story names an epic that has no epic file
And at least one story uses a referenced `task_file`
And at least one story has a sibling `.tasks.md`
And the status distribution covers every board column plus a status alias
```

**Scenario 3: Expensive read-path operations are countable**

```gherkin
Given a test holding a read-path counter handle
When it performs a repository read
Then it can assert the exact number of git root resolutions, settings parses,
    story parses and epic parses that occurred
And the counters are absent from a release build
```

**Scenario 4: Endpoint timings are reproducible**

```gherkin
Given a running kanban web server
When `scripts/benchmark_web_load.py` is run
Then it reports min, median, p95 and max over at least 20 runs per endpoint
And it reports response sizes
And it includes a concurrent board-plus-dashboard scenario
```

---

## Non-Functional Requirements

| Area | Requirement |
| ---- | ----------- |
| **Reproducibility** | No test or benchmark may depend on a repository outside this workspace |
| **Test hygiene** | No wall-clock timing assertion in the normal test suite |
| **Release hygiene** | Fixture generation and counters must not compile into a release binary |

---

## Technical Notes

- **Requirement refs:** `EP-004#acceptance-criteria`
- **Component / Module:** `crates/core/src/testsupport.rs`,
  `crates/core/src/instrument.rs`, `crates/web-server/src/bench.rs`,
  `scripts/benchmark_web_load.py`
- **Feature gating:** both modules are behind `#[cfg(any(test, feature = "test-support"))]`.
  `crates/web-server` enables `kanban-core/test-support` as a **dev**-dependency,
  so `cargo build` never sees them. In a release build the recording functions
  are inlined no-ops declared in `lib.rs`.
- **Why thread-local counters:** test binaries run tests concurrently, and a
  process-global counter is corrupted by unrelated tests building their own
  fixtures. Every counted read path is synchronous on the calling thread, so a
  thread-local count is exact and needs no locking.
- **Why an ignored `#[test]` for the no-HTTP benchmark:** it can reach the
  crate-private read model without widening any visibility, and `--ignored`
  keeps wall-clock work out of the normal suite.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [x] `FixtureSpec::representative()` and `FixtureSpec::minimal()` generate
      `git init`-ed fixtures with explicitly pinned feature flags
- [x] `ReadPathCounters` exposes git root resolutions, settings parses, story
      parses and epic parses
- [x] `scripts/benchmark_web_load.py` reports p95 and payload sizes with a
      concurrent scenario
- [x] `cargo test -p kanban-web-server --release -- --ignored --nocapture read_path_bench`
      reports cold read-model build, `doctor` and `validate` timings
- [x] No test depends on an external checkout
- [x] Full verification suite passes

---

## Dependencies

| Dependency | Type | Status | Notes |
| ---------- | ---- | ------ | ----- |
| None | - | - | Landed before any optimization commit so before/after numbers are comparable |

---

## Notes and Open Questions

| #   | Question / Assumption | Owner | Due | Resolved |
| --- | --------------------- | ----- | --- | -------- |
| None | - | - | - | - |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
