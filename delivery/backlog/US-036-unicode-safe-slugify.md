---
id: US-036
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

# User Story: Unicode-safe slugify

---

## Story Statement

**As a** user authoring Norwegian headlines,
**I want** `slugify` to preserve Unicode alphanumerics,
**so that** "Bærekraft" does not become the unrecognizable "bkraft".

---

## Background

`crates/core/src/util.rs:98-112` and `crates/web-server/src/lib.rs:1688` use
`to_ascii_lowercase`/`is_ascii_alphanumeric`, stripping all non-ASCII characters.

**Complexity: simple** — use `char::is_alphanumeric` and Unicode case folding.

---

## Acceptance Criteria

**Scenario 1: Norwegian headline slug preserves letters**

```gherkin
Given the headline "Bærekraft i 2026"
When `slugify` produces a slug
Then it is "bærekraft-i-2026" (or a documented transliteration)
```

**Scenario 2: ASCII headlines unchanged**

```gherkin
Given "Foundation Sprint"
When `slugify` runs
Then it is "foundation-sprint" as before
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Correctness**  | Non-ASCII alphanumerics are preserved or transliterated, not dropped |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/util.rs`, `crates/web-server/src/lib.rs` (delete after US-022/US-041)
- **Suggested patterns:** Use `char::is_alphanumeric()` and `to_lowercase()`; decide whether to transliterate (æ→ae) or preserve. Pairs with US-041.

### Estimation Rules

`story_points` is `2` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `slugify` preserves Unicode alphanumerics
- [ ] Norwegian headline test passes
- [ ] ASCII behavior unchanged
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-041: Consolidate slugify variants    | Story   | Draft     | Unify the two copies                    |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Update `slugify` to Unicode-aware char classification
- Add Norwegian headline test
- Decide transliteration policy

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Preserve (bærekraft) or transliterate (baerekraft)?             | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
