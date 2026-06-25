---
id: US-017
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 2
work_started: 2026-06-24T18:07:26+0200
work_done: 2026-06-25T09:08:45+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-25T09:08:45+0200
---

# User Story: Validate sprint name segment before filesystem join

---

## Story Statement

**As a** developer using the web API,
**I want** the `/api/sprints/{name}` route to validate the `name` segment against
the sprint naming grammar before any filesystem join,
**so that** a crafted `name` cannot produce a path outside the sprints directory
or rename an arbitrary file.

---

## Background

`crates/web-server/src/lib.rs:1421-1462` does
`config.sprints_path().join(format!("{name}.md"))` where `name` comes from the
URL. There is no validation against the `S<num>.<slug>` grammar; the handler
also `fs::rename`s. A crafted `name` could yield `sprints/../settings.md`.

**Complexity: simple** — validate `name` against a regex and reject `/`, `\`,
`..`, NUL.

---

## Acceptance Criteria

**Scenario 1: Valid sprint name is accepted**

```gherkin
Given a request to `/api/sprints/S1.foundation-sprint`
When the handler resolves the path
Then it joins `S1.foundation-sprint.md` under the sprints directory
```

**Scenario 2: Invalid name is rejected with 400**

```gherkin
Given a request to `/api/sprints/..%2Fsettings` or `/api/sprints/S1/evil`
When the handler validates the name
Then it returns 400 and performs no filesystem operation
```

**Scenario 3: Rename uses validated names only**

```gherkin
Given a sprint rename request with a crafted new name
When the handler computes the new path
Then the new name is validated first and rejected if unsafe
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Security**     | No `name` containing `/`, `\`, `..`, or NUL reaches a path join      |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/web-server/src/lib.rs` (`update_sprint_file` and route)
- **Suggested patterns:** Validate `name` against `^S[0-9]+\.[A-Za-z0-9-]+$`; reject otherwise with 400. Apply the same to the derived `new_name` in the rename path.
- **Testing approach:** Request `/api/sprints/..%2Fsettings` and assert 400 with no file change.

### Estimation Rules

`story_points` is `2` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `name` validated against sprint grammar before join
- [ ] Unsafe names return 400
- [ ] Test covers traversal and separator attempts
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-008: Path containment helper         | Story   | Draft     | Defense in depth; this is the route-level check |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add sprint-name validation helper
- Apply in `update_sprint_file` and route
- Add traversal-attempt test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should the grammar live in `core` so the CLI reuses it?          | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
