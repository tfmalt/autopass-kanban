---
id: US-034
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 3
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Static asset caching and Range headers

---

## Story Statement

**As a** user of the kanban web UI,
**I want** static assets served with `Cache-Control`, `ETag`, and `Accept-Ranges`,
**so that** the browser caches them and large payloads support range requests.

---

## Background

`crates/web-server/src/lib.rs:663-684` serves embedded files with a
`Content-Type` only — no `Cache-Control`, no `ETag`, no Range support.
Traversal-safe (include_dir), but uncacheable.

**Complexity: low** — add headers; `ETag` from embedded content hash at build time.

---

## Acceptance Criteria

**Scenario 1: Assets are cached**

```gherkin
Given a request for a static asset
When the response is returned
Then it includes `Cache-Control: public, max-age=300` (or immutable for hashed Vite assets)
And an `ETag` header
```

**Scenario 2: Range requests are supported**

```gherkin
Given a request with `Range: bytes=0-1023`
When the server responds
Then it returns `206 Partial Content` with `Accept-Ranges: bytes`
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Performance**  | Static assets are cacheable and rangeable                            |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` (`static_asset`), `build.rs`
- **Suggested patterns:** Compute `ETag` from the embedded file bytes (or a build-time hash); add `Cache-Control` based on whether the filename looks hashed; implement range slicing.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `Cache-Control`, `ETag`, and `Accept-Ranges` headers present
- [ ] Range requests return 206
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| None                                    | -       | -         | Standalone                             |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add `Cache-Control`/`ETag`/`Accept-Ranges` to `static_asset`
- Implement range slicing
- Add header tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
