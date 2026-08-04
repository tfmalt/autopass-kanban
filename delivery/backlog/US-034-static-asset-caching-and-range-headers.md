---
id: US-034
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 3
work_started: 2026-08-04T09:50:15+0200
work_done: 2026-08-04T09:50:15+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-08-04T09:50:15+0200
---

# User Story: Static asset caching and Range headers

---

## Story Statement

**As a** user of the kanban web UI,
**I want** static assets served with `Cache-Control`, `ETag`, and `Accept-Ranges`,
**so that** the browser caches them and large payloads support range requests.

---

## Background

`static_asset` (now `crates/web-server/src/handlers/mod.rs`, delegating to
`crates/web-server/src/static_assets.rs`) served embedded files with a
`Content-Type` only — no `Cache-Control`, no `ETag`, no Range support.
Traversal-safe (`include_dir`), but uncacheable. It also copied every asset's
bytes into a fresh `Vec` per request, and answered unknown `/api/*` paths with
`200 index.html` instead of `404`.

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

**Scenario 3: An unchanged asset transfers no bytes**

```gherkin
Given a client that already holds a hashed asset
When it revalidates with `If-None-Match` carrying the stored ETag
Then the server returns `304 Not Modified` with no body
And the `ETag` and `Cache-Control` headers are repeated
```

**Scenario 4: An unsatisfiable range is rejected**

```gherkin
Given a request whose first-byte-position is past the end of the representation
When the server responds
Then it returns `416 Range Not Satisfiable`
And `Content-Range: bytes */<length>`
```

**Scenario 5: Encodings are negotiated and separately validated**

```gherkin
Given an asset with a build-time gzip representation
When a client sends `Accept-Encoding: gzip`
Then the server returns `Content-Encoding: gzip`
And an `ETag` distinct from the identity representation's
And `Vary: Accept-Encoding`
```

**Scenario 6: Unknown API paths are not the SPA document**

```gherkin
Given a request for an `/api/*` path with no route
When the server responds
Then it returns `404`
And not `200` with `index.html`
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Performance**  | Static assets are cacheable and rangeable                            |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/static_assets.rs`, `crates/web-server/build.rs`
- **Implemented approach:** `build.rs` fingerprints every embedded asset with a
  SHA-256 strong `ETag` and writes a sorted `ASSET_MANIFEST`. Compressible assets
  above 1 KiB that gain at least 10% are gzipped at build time into a sibling
  `.gz` representation with its own `ETag`.
- **Why build time, not a runtime `CompressionLayer`:** the assets are embedded
  and cannot change at runtime, so compressing once per build beats once per
  request; it produces a stable strong validator per representation; it lets a
  range be served against the representation the client actually negotiated,
  avoiding the range-versus-compression interaction; and it keeps `tower-http`
  out of the dependency tree. `flate2` and `sha2` are `[build-dependencies]`
  only and do not reach the shipped binary.
- **Cache policy:** `immutable` for content-hashed `/assets/*`, `no-cache` for
  `index.html` (it is the document that names the hashed assets, so it must be
  revalidated or a new build stays invisible), `max-age=300` otherwise.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [x] `Cache-Control`, `ETag`, and `Accept-Ranges` headers present
- [x] Range requests return 206; unsatisfiable ranges return 416; `If-Range` is honored
- [x] `If-None-Match` returns 304
- [x] `Content-Encoding` negotiated with `Vary: Accept-Encoding`
- [x] `Bytes::from_static` replaces the per-request asset copy
- [x] Unknown `/api/*` paths return 404 instead of the SPA document
- [x] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [x] `kanban validate .` and `kanban doctor .` pass
- [x] Workspace version bumped in `Cargo.toml`

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
