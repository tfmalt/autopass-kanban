---
id: US-045
type: user-story
status: done
epic: EP-004
sprint: S001.rolling-thunder
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
priority: 40
work_started: 2026-08-04T10:00:13+0200
work_done: 2026-08-04T10:00:13+0200
created: 2026-08-04T10:00:13+0200
updated: 2026-08-27T10:06:07+0200
activated: 2026-08-27T10:06:07+0200
---

# User Story: Coalesce filesystem events and give SSE resumable identity

---

## Story Statement

**As a** user with the kanban board open,
**I want** one source-change burst to produce one identified live-reload event,
**so that** a `git pull` does not trigger dozens of full refetches and a dropped
connection does not silently lose changes.

---

## Background

This is a correctness defect, not only a performance one.

The watcher called `events.send(())` on every raw `notify` event with no
debouncing, so one `git pull` or one multi-file `kanban` write fanned out into
many `change` events, and the client invalidated three query keys per event with
no throttle.

Worse, the events carried **no id** and there was no `Last-Event-ID` handling.
`EventSource` reconnects automatically, but every change during the gap was lost
permanently. `RecvError::Lagged(_)` silently discarded events for a slow
subscriber. Exceeding `SSE_SUBSCRIBER_CAP` returned `503`, and the client
registered no error handler, so a capped-out client got no live reload and no
indication that it had lost it.

Separately, the SSE subscriber guard was a local binding in the handler, so it
was dropped as soon as the handler returned the response — the cap counted
concurrently *executing handlers*, not concurrently *open streams*, and therefore
bounded nothing.

Finally, `axum::serve(..).with_graceful_shutdown(..)` stops accepting new
connections and then waits for the in-flight ones to finish. An SSE stream never
finishes on its own, so the server survived SIGTERM for as long as any browser
tab held `/api/events` open. `kanban web stop` therefore always spent its full
3 s `wait_for_process_exit` window and then fell through to SIGKILL whenever the
UI was in use. Confirmed by observation, not inference: after SIGTERM the process
remained alive with its listening socket closed and one `ESTABLISHED` connection
to the browser.

---

## Acceptance Criteria

**Scenario 1: A burst produces one event**

```gherkin
Given a live-reload subscriber
When 50 raw filesystem events arrive inside the debounce window
Then exactly one `change` event is published
```

**Scenario 2: A sustained burst still publishes**

```gherkin
Given filesystem events arriving continuously for longer than the ceiling
When the ceiling elapses
Then a change is published without waiting for the burst to end
```

**Scenario 3: Events are resumable**

```gherkin
Given a published change
Then it carries a strictly increasing generation as its SSE event id
And generations never repeat
```

**Scenario 4: A reconnect gap is detected**

```gherkin
Given a client that reconnects with a `Last-Event-ID` behind the current generation
When the stream opens
Then it immediately receives a `resync` event at the current generation
And a client already at the current generation receives nothing
```

**Scenario 5: A lagged subscriber is told to refetch**

```gherkin
Given a subscriber that falls behind the broadcast buffer
When it next reads the stream
Then it receives a `resync` event rather than silently continuing
```

**Scenario 6: The subscriber cap bounds open streams**

```gherkin
Given an open live-reload stream
Then it counts against `SSE_SUBSCRIBER_CAP` for as long as it stays open
And releases its slot when it closes
```

**Scenario 7: Shutdown is not blocked by an open stream**

```gherkin
Given a browser tab holding an open live-reload stream
When the server receives SIGTERM
Then the stream ends and the connection closes
And the process exits without needing SIGKILL
```

---

## Non-Functional Requirements

| Area | Requirement |
| ---- | ----------- |
| **Correctness** | A client is never left silently stale after a change it did not observe |
| **Responsiveness** | A sustained burst publishes at most one ceiling interval late |
| **Test determinism** | Coalescing is asserted under a controlled clock, never with real sleeps |

---

## Technical Notes

- **Requirement refs:** `EP-004#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/changes.rs` (new), `lib.rs`,
  `handlers/mod.rs`
- **Design:** a capacity-1 `mpsc` channel makes coalescing structural — a burst
  collapses into at most one queued signal — feeding a debounce loop with a
  150 ms quiet window and a 1 s ceiling. The ceiling arm is `biased` first in the
  `select!` so a sustained burst cannot starve clients.
- **Ordering:** the generation is incremented and broadcast only after the burst
  completes, so a subscriber reacting to generation N observes every change that
  produced it.
- **Deliberately not debounced:** `branch_cache` invalidation stays on the raw
  event. It is a single `Option` reset, and delaying it would serve a stale
  branch name for up to a second after a checkout.
- **Also watched:** `.kanban/settings.json`, since configuration changes alter
  which files are served and how they are interpreted.
- **Shutdown:** a `tokio::sync::watch<bool>` is set by the signal handler
  *before* it resolves, because axum only starts waiting for in-flight
  connections after that future completes. The SSE stream selects on it and
  ends. Measured: the process now exits 0.02 s after SIGTERM with a stream open,
  where it previously never exited on its own.
- **Testing:** `tokio::test(start_paused = true)` with `tokio/test-util` as a
  dev-dependency; tests observe publication through a probe subscriber rather
  than guessing scheduler turns.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [x] No code path calls `events.send` outside the coalescer
- [x] Every SSE event carries a monotonic id
- [x] `Last-Event-ID` and broadcast lag both trigger an explicit resync
- [x] The subscriber guard lives in the stream, not the handler
- [x] An open stream no longer blocks graceful shutdown
- [x] Deterministic tests under a paused clock, no real sleeps
- [x] Full verification suite passes

---

## Dependencies

| Dependency | Type | Status | Notes |
| ---------- | ---- | ------ | ----- |
| None | - | - | Independent of the read-path work; a prerequisite for US-046 |
| US-015 | Story | Done | `kanban web stop` relies on the process exiting on SIGTERM |

---

## Notes and Open Questions

| #   | Question / Assumption | Owner | Due | Resolved |
| --- | --------------------- | ----- | --- | -------- |
| 1 | Should shutdown be handled here or in `US-015`? Handled here: the blocker is the SSE stream's lifetime, which this story owns | Tooling lead | 2026-08-04 | Yes |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
