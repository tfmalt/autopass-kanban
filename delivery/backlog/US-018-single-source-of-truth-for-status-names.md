---
id: US-018
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T16:22:42+0200
work_done: 2026-06-24T16:28:39+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:28:39+0200
---

# User Story: Single source of truth for story and task status names

---

## Story Statement

**As a** maintainer adding a new story or task status,
**I want** the status name list to be defined in exactly one place consumed by
clap, completion, theme, render, and validate,
**so that** adding a status cannot silently leave one consumer out of sync.

---

## Background

The story-status list (`draft backlog ready todo in-progress ready-for-qa blocked
done dropped`) and task-status list (`todo in-progress blocked done`) appear as
bare string literals in ~10 places: `completion.rs:91-96,170-184,924,965,1010`,
`cli.rs:260`, `theme.rs:146-161`, `render/common.rs:20-23`, `render/phase.rs`,
`render/epic.rs:84`, and `validate.rs`. AGENTS.md warns against adding status
names without checking existing usage.

**Complexity: medium** — define consts, thread them through ~10 call sites, add
a test that asserts all consumers use the same list.

---

## Acceptance Criteria

**Scenario 1: One const defines the story statuses**

```gherkin
Given the codebase after the change
When grepping for the status list as a bare literal
Then only the single `const STORY_STATUSES: &[&str]` definition remains
And all other consumers reference it
```

**Scenario 2: Adding a status updates all consumers**

```gherkin
Given a maintainer adds `cancelled` to `STORY_STATUSES`
When the project builds
Then clap, completion, theme colors, render icons, and validate all recognize `cancelled` with no further edits
```

**Scenario 3: Completion helpers use the const**

```gherkin
Given the zsh and bash completion scripts
When they are generated
Then the dynamic status helpers emit exactly the const's values
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Maintainability** | No duplicated status-name literals remain                            |
| **AGENTS.md**       | Complies with the "check existing usage" rule by construction         |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/constants.rs` (new consts), `crates/cli/src/{cli.rs,completion.rs,theme.rs,render/*}`, `crates/core/src/validate.rs`
- **Suggested patterns:** Define `pub const STORY_STATUSES: &[&str] = &[...]` and `pub const TASK_STATUSES: &[&str] = &[...]` in `constants.rs`; derive clap `ValueEnum` possible values from them; inject into completion raw strings and theme/render match arms.
- **Testing approach:** A test asserts every consumer's status set equals `STORY_STATUSES`/`TASK_STATUSES`.
- **Migration / backward compatibility:** No status names are added or removed — only consolidated.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `STORY_STATUSES` and `TASK_STATUSES` consts exist in `core`
- [ ] clap, completion, theme, render, and validate all reference the consts
- [ ] Test asserts consumer consistency
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-019: Completion round-trip tests     | Story   | Draft     | Validates the completion consumer       |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Define status consts in `constants.rs`
- Replace bare literals across consumers
- Derive clap/completion/theme/render from the consts
- Add consistency test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should a `StoryStatus` enum replace the `&[&str]` const?         | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
