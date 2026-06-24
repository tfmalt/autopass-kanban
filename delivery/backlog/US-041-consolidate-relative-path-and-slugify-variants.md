---
id: US-041
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

# User Story: Consolidate relative_path and slugify variants

---

## Story Statement

**As a** maintainer,
**I want** `relative_path` and `slugify` to each have a single implementation
in `core`,
**so that** the three slugify copies and two `relative_path` copies cannot
diverge.

---

## Background

`relative_path` is defined twice (`util.rs:144` and `config.rs:668`), and there
are three `slugify` variants (`util::slugify_headline`, `json::slugify_status`,
`web-server::slugify`).

**Complexity: low** — consolidate to one of each in `core`.

---

## Acceptance Criteria

**Scenario 1: One relative_path implementation**

```gherkin
Given the refactored core
When grepping for `fn relative_path`
Then exactly one definition exists in `core` and others call it
```

**Scenario 2: One slugify implementation**

```gherkin
Given the refactored codebase
When grepping for `fn slugify`
Then exactly one definition exists in `core` and callers (including the web server) use it
```

**Scenario 3: No behavior regression**

```gherkin
Given the consolidation
When existing slugify and relative_path tests run
Then they pass unchanged
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Maintainability** | `relative_path` and `slugify` each have one source of truth           |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/util.rs`, `crates/core/src/config.rs:668`, `crates/core/src/json.rs:258`, `crates/web-server/src/lib.rs:1688`
- **Suggested patterns:** Keep `util::relative_path` and `util::slugify` (made Unicode-safe by US-036); have `config.rs` and `json.rs` call them; delete the web-server copy (also via US-022).
- **Testing approach:** Existing tests; add a test that all callers produce identical output.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Single `relative_path` in `core`
- [ ] Single `slugify` in `core` (Unicode-safe per US-036)
- [ ] `config.rs`, `json.rs`, and web-server callers updated
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-036: Unicode-safe slugify            | Story   | Draft     | Provides the canonical implementation  |
| US-022: Move markdown helpers to core   | Story   | Draft     | Removes the web-server slugify copy    |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Remove duplicate `relative_path` in `config.rs`
- Replace `json::slugify_status` and `web-server::slugify` with calls to `util::slugify`
- Add consistency test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
