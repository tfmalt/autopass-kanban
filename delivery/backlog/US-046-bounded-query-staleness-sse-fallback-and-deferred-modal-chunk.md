---
id: US-046
type: user-story
status: done
epic: EP-004
sprint: S001.rolling-thunder
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
priority: 50
work_started: 2026-08-04T10:00:13+0200
work_done: 2026-08-04T10:00:13+0200
created: 2026-08-04T10:00:13+0200
updated: 2026-08-27T10:06:07+0200
activated: 2026-08-27T10:06:07+0200
---

# User Story: Bounded query staleness, SSE fallback, and deferred modal chunk

---

## Story Statement

**As a** user of the kanban web UI,
**I want** the client to stop refetching redundantly without ever showing me
stale data,
**so that** the UI stays fast and trustworthy even when live reload is lost.

---

## Background

The obvious way to stop redundant refetches is `staleTime: Infinity`
with focus and mount refetching disabled, making SSE the sole freshness
mechanism. Given the reliability holes described in `US-045`, that would convert
each of them into permanent, silent staleness — the worst possible failure for a
tool whose entire value is showing current state.

Separate avoidable client work: `AppShell` fetched the whole 687 KB repository
snapshot to read four numbers out of `progress`; `useTeam` was called inside the
card component so every rendered card created its own query observer and `Map`;
`DashboardView` checked only `metrics` for loading and errors, so a failed
repository fetch silently rendered an empty phase section; `gitPull` called an
unfiltered `invalidateQueries()` that also discarded `["config"]` and every
`["story", id]`; and `StoryModal` plus DOMPurify (~129 KB raw) were static
dependencies of the board and backlog route chunks, so they downloaded on every
board visit even if no card was ever opened.

---

## Acceptance Criteria

**Scenario 1: Freshness is bounded, not infinite**

```gherkin
Given a query that resolved less than 60 seconds ago
When its component remounts
Then it is served from cache without a refetch
And once the data is stale a refetch occurs
```

**Scenario 2: Losing live reload is visible and self-healing**

```gherkin
Given an open live-reload stream
When the stream errors or the server refuses the subscription
Then a non-blocking "live updates unavailable" indicator appears
And the aggregate queries fall back to periodic refresh
And both are withdrawn when the stream recovers
```

**Scenario 3: One change costs one refetch per key**

```gherkin
Given a live-reload `change` event
Then each aggregate query key is invalidated exactly once
```

**Scenario 4: Sync does not discard unrelated caches**

```gherkin
Given a successful `git pull` from the web UI
Then only the aggregate query keys are invalidated
And `["config"]` and story-detail entries are left intact
```

**Scenario 5: The story modal is deferred**

```gherkin
Given a cold board load
Then the story modal chunk is not requested
And it is requested only when a card is opened
```

**Scenario 6: Loading states do not shift the layout**

```gherkin
Given a cold board or dashboard load
Then a fixed-dimension skeleton matching the real layout is shown
And it exposes an accessible busy status
And cumulative layout shift stays at or below 0.02
And a background refresh keeps the previous content on screen
```

---

## Non-Functional Requirements

| Area | Requirement |
| ---- | ----------- |
| **Correctness** | Losing SSE must never leave a client permanently stale |
| **Accessibility** | Loading placeholders expose `role="status"` and `aria-busy` |
| **Performance** | CLS <= 0.02; the board startup graph excludes the modal and its sanitizer |
| **Motion** | Skeleton animation is disabled under `prefers-reduced-motion` |

---

## Technical Notes

- **Requirement refs:** `EP-004#acceptance-criteria`
- **Component / Module:** `web/src/main.tsx`, `web/src/api/hooks.ts`,
  `web/src/components/{AppShell,StoryCard,StoryColumn,Skeletons}.tsx`,
  `web/src/views/{BoardView,BacklogView,DashboardView}.tsx`,
  `web/src/styles/app.css`
- **Query policy:** `staleTime: 60_000`, `refetchOnWindowFocus: true`,
  `refetchOnMount: false`, `refetchOnReconnect: true`. A bounded staleness plus
  SSE gives the same practical freshness as `Infinity` while degrading
  gracefully when SSE is lost. Now that the server answers in tens of
  milliseconds, a refetch on focus is not worth optimizing away. `useConfig`
  keeps `staleTime: Infinity` because configuration changes arrive through the
  watcher.
- **`keepPreviousData`** keeps the last good render on screen during a
  background refetch instead of unmounting to a loading state, which is what
  produced a layout shift on every live-reload event.
- **Team lookup** is resolved once per board by `useAssigneeMap` and passed to
  cards as a prop.
- **Client-side coalescing** batches invalidation to one animation frame as a
  backstop, even though the server already coalesces.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [x] Bounded `staleTime` with focus refetch and an SSE `onerror` fallback
- [x] The unfiltered `invalidateQueries()` after `gitPull` is replaced with explicit keys
- [x] `useTeam` is hoisted out of `StoryCard`
- [x] The story modal and its sanitizer are deferred out of board startup
- [x] Fixed-dimension accessible skeletons; background refresh keeps content
- [x] `DashboardView` handles its repository data source explicitly
- [x] `npm --prefix web run typecheck`, `test` and `build` pass
- [x] Full verification suite passes

---

## Dependencies

| Dependency | Type | Status | Notes |
| ---------- | ---- | ------ | ----- |
| US-045 | Story | Done | Defines the SSE contract this client consumes |

---

## Notes and Open Questions

| #   | Question / Assumption | Owner | Due | Resolved |
| --- | --------------------- | ----- | --- | -------- |
| 1 | Should `staleTime: Infinity` be used, with SSE as the sole freshness mechanism? Rejected; it converts every SSE reliability hole into silent permanent staleness | Tooling lead | 2026-08-04 | Yes |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
