---
id: US-014
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T16:05:39+0200
work_done: 2026-06-24T16:20:12+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:20:12+0200
---

# User Story: CSRF protection for web-server mutation endpoints

---

## Story Statement

**As a** developer running `kanban web start`,
**I want** the web server to reject non-GET requests whose `Origin`/`Host` does
not match the bound address,
**so that** a malicious web page I visit cannot silently drive mutation
endpoints against my local kanban server.

---

## Background

`crates/web-server/src/lib.rs:338-358` registers POST/PATCH/PUT handlers with
no auth, no CSRF token, and no `Origin`/`Host` validation. The default 127.0.0.1
binding plus no CORS currently limits exposure to preflighted requests, but the
safety is implicit and fragile — any future permissive CORS change would make
this exploitable.

**Complexity: medium** — an axum middleware checking `Origin`/`Host` against the
bound address for non-GET methods, plus tests.

---

## Acceptance Criteria

**Scenario 1: Same-origin mutation succeeds**

```gherkin
Given the server is bound to `127.0.0.1:8080`
When a browser on the kanban UI issues `POST /api/stories/<id>/move` with `Origin: http://127.0.0.1:8080`
Then the request is accepted
```

**Scenario 2: Cross-origin mutation is rejected**

```gherkin
Given the server is bound to `127.0.0.1:8080`
When a page at `http://evil.example` issues `POST /api/stories/<id>/move` with `Origin: http://evil.example`
Then the server returns 403 and no mutation occurs
```

**Scenario 3: Missing Origin on mutation is rejected**

```gherkin
Given a non-GET request with no `Origin` and no `Referer` header
When it reaches a mutation endpoint
Then the server returns 403
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Security**     | All non-GET methods require a matching `Origin` or `Referer`          |
| **Backward compat** | The kanban SPA continues to work unchanged                         |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` (router + new middleware)
- **Suggested patterns:** Add an axum `from_fn` middleware that compares `Origin` (falling back to `Referer`) host:port to the bound server address for non-GET methods; return 403 on mismatch. GET endpoints are unaffected.
- **Testing approach:** Unit/integration tests issuing mutation requests with matching, mismatching, and absent `Origin` headers.
- **Migration / backward compatibility:** The SPA already sends same-origin requests, so it continues to work.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Middleware rejects non-GET requests with mismatched/absent `Origin`/`Referer`
- [ ] Tests cover matching, mismatching, and absent origin
- [ ] SPA continues to function
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| None                                    | -       | -         | Standalone hardening                   |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add `Origin`/`Host` allow-list middleware
- Wire it for non-GET methods
- Add origin-matching tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should an instance-bound token header be added in addition?      | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
