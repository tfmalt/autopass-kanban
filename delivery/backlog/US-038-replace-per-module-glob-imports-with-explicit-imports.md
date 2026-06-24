---
id: US-038
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

# User Story: Replace per-module glob imports with explicit imports

---

## Story Statement

**As a** maintainer,
**I want** each `cli/src` module to import only what it uses instead of
glob-importing the whole crate,
**so that** the unused-import lint can catch dead imports and namespaces stay
clean.

---

## Background

Every module in `crates/cli/src/` begins with
`#[allow(unused_imports)] use crate::{cli::*, completion::*, doctor_cli::*, json_out::*, prompt::*, render::*, self_manage::*, theme::*, web::*};`,
blanket-allowing the lint and polluting namespaces.

**Complexity: low** — mechanical replacement of globs with explicit imports.

---

## Acceptance Criteria

**Scenario 1: No mega-globs remain**

```gherkin
Given the refactored `crates/cli/src/`
When grepping for `use crate::{cli::*, completion::*`
Then no module glob-imports the whole crate
```

**Scenario 2: Unused-import lint is active**

```gherkin
Given the refactored crate
When `cargo clippy` runs
Then the `unused_imports` lint is no longer blanket-allowed and reports real unused imports
```

**Scenario 3: Build still succeeds**

```gherkin
Given the refactored imports
When `cargo build` runs
Then it succeeds
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Maintainability** | Imports are explicit; the unused-import lint is effective            |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** every file in `crates/cli/src/` and `render/*`
- **Suggested patterns:** Use a `prelude` module for the genuinely shared primitives and explicit `use` lines per module; rely on rust-analyzer/`cargo fix` to add imports.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Mega-glob imports removed
- [ ] Each module imports only what it uses
- [ ] `unused_imports` lint no longer blanket-allowed
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

- Remove mega-globs per module
- Add explicit imports (use `cargo fix` to assist)
- Verify lint is active

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
