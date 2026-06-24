---
id: US-025
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 8
work_started: 2026-06-24T11:17:16+0200
work_done: 2026-06-24T16:38:04+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:38:04+0200
---

# User Story: Typed KanbanError enum at the core boundary

---

## Story Statement

**As a** consumer of the JSON API,
**I want** the `code` field to derive from a typed error enum rather than
string-sniffing `anyhow` messages,
**so that** rewording an error message cannot silently change the code clients
receive.

---

## Background

`crates/core/src/json.rs:50-77` (`KanbanErrorCode::classify`) walks
`error.to_string().to_lowercase()` for substrings ("sprint not found",
"frontmatter", …). The public JSON contract is pinned to prose; any `bail!`
rewording silently changes the code.

**Complexity: high** — introduce a typed enum at the `core` public boundary,
return it from public functions, and match on it in the CLI. `anyhow` stays
internal.

---

## Acceptance Criteria

**Scenario 1: Error code derives from typed enum**

```gherkin
Given a `StoryNotFound` error returned from a core function
When the CLI emits the JSON envelope
Then `code` is `story_not_found` regardless of the message wording
```

**Scenario 2: anyhow stays internal**

```gherkin
Given the refactored core crate
When inspecting public function signatures
Then public functions return `Result<_, KanbanError>` (or a wrapper) and `anyhow` is used only internally
```

**Scenario 3: Message rewording does not change code**

```gherkin
Given a maintainer rewords the "sprint not found" `bail!` message
When the JSON envelope is emitted for that case
Then `code` is still `sprint_not_found`
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **API stability**   | The JSON `code` contract is independent of error message prose        |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/json.rs`, public functions across `crates/core`
- **Suggested patterns:** Define `pub enum KanbanError { NotInitialized, StoryNotFound, SprintNotFound, InvalidArgument, InvalidStatus, FeatureDisabled, Io(PathBuf), ... }`; implement `Display`/`Into<anyhow::Error>`; have `KanbanErrorCode::from(&KanbanError)`. Public core functions return `Result<T, KanbanError>`; internal helpers keep `anyhow`.
- **Testing approach:** Unit tests mapping each `KanbanError` variant to its code; integration tests asserting reworded messages keep the same code.
- **Migration / backward compatibility:** JSON codes are unchanged for existing cases; the change makes them stable.

### Estimation Rules

`story_points` is `8` (complexity: high) — touches every public `core` function
signature and the CLI error mapping.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `KanbanError` enum defined in `core`
- [ ] Public `core` functions return typed errors
- [ ] `KanbanErrorCode` derives from the enum, not string matching
- [ ] Tests assert code stability across message rewording
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-024: Semantic exit codes             | Story   | Draft     | Consumes the typed enum                 |
| US-020: Shared CLI orchestration        | Story   | Draft     | Builders can return typed errors        |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Define `KanbanError` enum and conversions
- Update public `core` signatures
- Replace `classify` string matching with enum matching
- Add stability tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | One shared enum or per-function error types?                    | Tooling lead | 2026-07-04 | Yes — one shared `pub enum KanbanError` for the crate, returned by all public `core` functions. `anyhow` stays internal. Matches the story's suggested pattern and keeps the `KanbanErrorCode` mapping in one place. |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
