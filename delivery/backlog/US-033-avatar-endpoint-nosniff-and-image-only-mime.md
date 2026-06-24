---
id: US-033
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 2
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Avatar endpoint nosniff and image-only MIME

---

## Story Statement

**As a** user of the kanban web UI,
**I want** the avatar endpoint to serve only image MIME types with
`X-Content-Type-Options: nosniff`,
**so that** a file dropped in `.kanban/team_avatars/evil.html` cannot execute
as a script in the app origin.

---

## Background

`crates/web-server/src/lib.rs:446-475` serves any file under
`.kanban/team_avatars` with a guessed MIME and no `nosniff`, allowing stored XSS
via a non-image file in the same origin.

**Complexity: simple** — add `nosniff` globally and reject non-image MIME on
the avatar route.

---

## Acceptance Criteria

**Scenario 1: Non-image avatar is rejected**

```gherkin
Given `.kanban/team_avatars/evil.html` exists
When the browser requests `/api/team/avatars/evil.html`
Then the server returns 415 (or 404) with `nosniff` and does not serve it as `text/html`
```

**Scenario 2: Image avatar is served with nosniff**

```gherkin
Given a `.png` avatar
When the browser requests it
Then the response has `Content-Type: image/png` and `X-Content-Type-Options: nosniff`
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Security**     | No same-origin script execution via the avatar endpoint              |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` (`api_team_avatar`, global middleware)
- **Suggested patterns:** Add a global `X-Content-Type-Options: nosniff` middleware; in `api_team_avatar`, return 415 when the guessed MIME is not `image/*`.

### Estimation Rules

`story_points` is `2` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `nosniff` set on all responses
- [ ] Avatar endpoint rejects non-image MIME
- [ ] Test covers `evil.html` rejection
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

- Add global `nosniff` middleware
- Restrict avatar MIME to `image/*`
- Add rejection test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
