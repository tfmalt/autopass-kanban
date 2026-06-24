---
id: US-022
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Move web-server markdown mutation helpers into kanban-core

---

## Story Statement

**As a** maintainer,
**I want** the web server to call `kanban_core::markdown` for all markdown
mutations instead of its own hand-rolled helpers,
**so that** the CLI and web server share one writer and cannot diverge in
semantics (e.g. CRLF handling).

---

## Background

`crates/web-server/src/lib.rs:1600-1688` defines `replace_markdown_body`,
`replace_frontmatter_fields`, `replace_section_content`, `replace_sprint_title`,
and `slugify`. These duplicate `kanban_core::markdown`'s
`update_story_frontmatter_markdown`/`upsert_frontmatter_markdown` with
divergent CRLF handling, violating the AGENTS.md rule "Keep backlog semantics
in `crates/core`". The web UI can write markdown the CLI validator rejects.

**Complexity: medium** — move helpers to `core`, expose signatures, rewrite
web-server callers, delete the copies.

---

## Acceptance Criteria

**Scenario 1: No markdown helpers remain in web-server**

```gherkin
Given the refactored codebase
When grepping `crates/web-server` for `replace_markdown_body`/`replace_frontmatter_fields`/`replace_section_content`/`replace_sprint_title`
Then none are found
```

**Scenario 2: Web server calls core for mutations**

```gherkin
Given a web-server mutation handler that edits frontmatter
When it updates markdown
Then it calls a `kanban_core::markdown` function
```

**Scenario 3: CRLF consistency**

```gherkin
Given a sprint file with CRLF line endings edited via the web UI
When the edit is saved
Then the file's line endings remain consistent (no mixed CRLF/LF)
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Architecture**    | All markdown semantics live in `crates/core` (AGENTS.md compliance)   |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/markdown.rs` (add `replace_section_content`, `replace_sprint_title`, body-replace), `crates/web-server/src/lib.rs` (delete copies, call core)
- **Suggested patterns:** Move `replace_section_content` and `replace_sprint_title` into `core::markdown`; make the web server's frontmatter edits use `upsert_frontmatter_markdown`. Unify the slugify variant with US-041.
- **Testing approach:** Reuse `core::markdown` tests; add a CRLF-consistency test for the web write path.
- **Migration / backward compatibility:** Web behavior aligns with CLI; any file the web UI previously wrote that failed CLI validation now validates.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `replace_section_content`/`replace_sprint_title` live in `core::markdown`
- [ ] Web server calls `core::markdown` for all mutations
- [ ] Web-server copies deleted
- [ ] CRLF-consistency test passes
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-021: Split web-server module         | Story   | Draft     | Easier to relocate after the split      |
| US-012: Atomic writes                   | Story   | Draft     | The shared writer should also be atomic |
| US-041: Consolidate slugify variants    | Story   | Draft     | Removes the web-server `slugify` copy   |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Move section/sprint-title helpers into `core::markdown`
- Rewrite web-server callers to use `core`
- Delete web-server copies
- Add CRLF-consistency test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should `replace_section_content` be public or crate-internal?    | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
