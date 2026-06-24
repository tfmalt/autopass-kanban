---
id: US-027
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

# User Story: Replace unwrap on user-controlled parent paths

---

## Story Statement

**As a** developer,
**I want** `parent().unwrap()` calls on user-controlled paths replaced with
propagated errors,
**so that** an edge-case path cannot panic the CLI.

---

## Background

`repository.rs:173,196,221` and `doctor.rs:176` call
`file_path.parent().unwrap()`, which panics if the path has no parent (e.g. a
canonicalized root path).

**Complexity: simple** — replace four `unwrap()`s with `?` + `with_context`.

---

## Acceptance Criteria

**Scenario 1: Root-level path does not panic**

```gherkin
Given a path that resolves to the filesystem root
When the writer resolves its parent
Then it returns an error naming the path instead of panicking
```

**Scenario 2: Normal paths still work**

```gherkin
Given a normal story path
When the writer resolves its parent
Then it succeeds as before
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Robustness**   | No `unwrap()`/`expect()` on user-derived `parent()` results          |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/repository.rs`, `crates/core/src/doctor.rs`
- **Suggested patterns:** `file_path.parent().with_context(|| format!("{} has no parent dir", file_path.display()))?`.

### Estimation Rules

`story_points` is `2` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] All four `parent().unwrap()` sites replaced with `?` + context
- [ ] No new panics on edge-case paths
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

- Replace `parent().unwrap()` with `?` + `with_context`
- Add edge-case path test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | - |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
