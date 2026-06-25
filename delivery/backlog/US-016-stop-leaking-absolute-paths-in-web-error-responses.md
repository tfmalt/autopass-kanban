---
id: US-016
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 2
work_started: 2026-06-24T18:05:02+0200
work_done: 2026-06-24T18:06:46+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T18:06:46+0200
---

# User Story: Stop leaking absolute filesystem paths in web error responses

---

## Story Statement

**As a** developer using the local web API,
**I want** error responses to omit absolute filesystem paths,
**so that** a remote caller cannot learn my home directory and repo layout from
HTTP error bodies.

---

## Background

`crates/web-server/src/lib.rs:708-727` converts every `anyhow::Error` to HTTP
422 with the full chain string as the JSON body. `with_context` calls embed
`Path::display()` of absolute paths, disclosing the developer's home directory.

**Complexity: simple** — log the full chain server-side; return a generic
message to the client.

---

## Acceptance Criteria

**Scenario 1: Error body contains no absolute paths**

```gherkin
Given a mutation handler returns an `anyhow::Error` with path context
When the response is serialized
Then the JSON body does not contain any absolute filesystem path
```

**Scenario 2: Full error is logged server-side**

```gherkin
Given the same error
When it is handled by the response converter
Then the full `anyhow` chain (including paths) is written to stderr/server log
```

**Scenario 3: Explicit 400/404 messages are preserved**

```gherkin
Given a handler constructs an explicit "story not found" 404
When the response is returned
Then the client still receives the specific message (only generic for propagated `anyhow` errors)
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Security**     | No absolute path, username, or machine name in any HTTP response body |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` (`ApiResponse::from(anyhow::Error)`)
- **Suggested patterns:** In the `From<anyhow::Error>` impl, `eprintln!`/`tracing::error!` the full chain and return a generic `"internal error"` (or a coarse category) to the client. Keep explicitly-constructed 400/404 responses as-is.
- **Testing approach:** Assert an error response body does not contain `/Users/` or `/home/`.

### Estimation Rules

`story_points` is `2` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Propagated `anyhow` errors produce generic client messages
- [ ] Full error chain logged server-side
- [ ] Explicit 400/404 messages preserved
- [ ] Test asserts no absolute path in response body
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

- Change `From<anyhow::Error>` to log + generic message
- Add response-body leak test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should a coarse error category be returned instead of "internal error"? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
