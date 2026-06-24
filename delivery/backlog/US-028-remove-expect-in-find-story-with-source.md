---
id: US-028
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 1
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Remove expect in find_story_with_source

---

## Story Statement

**As a** developer,
**I want** `find_story_with_source` to return `Result<Option<...>>` instead of
`.expect()`,
**so that** a story disappearing between two repository passes cannot panic the
CLI.

---

## Background

`crates/core/src/story.rs:547-548` uses
`.expect("story was found in the same repository scan")`. If a story is deleted
between the two passes or the predicates diverge, this panics on user data.

**Complexity: simple** — propagate the `None` as an error.

---

## Acceptance Criteria

**Scenario 1: Vanished story returns an error, not a panic**

```gherkin
Given a story that disappears between the two repository passes
When `find_story_with_source` runs
Then it returns an error naming the vanished story
```

**Scenario 2: Normal lookup still works**

```gherkin
Given a present story
When `find_story_with_source` runs
Then it returns the story details as before
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Robustness**   | No `.expect()` on user data in `find_story_with_source`              |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/story.rs:547-548`
- **Suggested patterns:** Return `Result<Option<StoryDetails>>` and map `None` to a `bail!("story {id} vanished during scan")` at the caller.

### Estimation Rules

`story_points` is `1` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `.expect()` replaced with propagated error
- [ ] Caller handles `None` gracefully
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

- Change `find_story_with_source` return type
- Update caller to handle `None`

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | - |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
