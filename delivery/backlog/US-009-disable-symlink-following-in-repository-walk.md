---
id: US-009
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T16:34:51+0200
work_done: 2026-06-24T16:36:01+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:36:01+0200
---

# User Story: Disable symlink following in the repository walk

---

## Story Statement

**As a** developer with a backlog directory that may contain planted symlinks,
**I want** `kanban` to not follow symlinks when scanning for story files,
**so that** a symlinked `US-*.md` cannot resolve outside the backlog root and
cause the writer to write through it to an arbitrary location.

---

## Background

`crates/core/src/repository.rs:15-31` uses `WalkDir::new(&backlog_root)` which
follows symlinks by default for files. A symlinked story file resolves via
`fs::canonicalize` to a path outside `repo_root`, and the subsequent
`fs::write(&story.file_path, ...)` in `move_story_to_status` writes through the
symlink. Combined with US-008's containment check this is defense in depth, but
the walk itself should not follow links.

**Complexity: medium** — small code change plus a test that plants a symlink and
asserts it is skipped.

---

## Acceptance Criteria

**Scenario 1: Symlinked story file is not scanned**

```gherkin
Given the backlog root contains a symlink `US-F1-999.md` pointing to `/tmp/evil.md`
When `kanban story list --all` runs
Then the symlinked entry does not appear in the story list
And no write reaches `/tmp/evil.md`
```

**Scenario 2: Real story files still scanned**

```gherkin
Given a backlog with regular `US-*.md` files and one symlink
When `kanban story list --all` runs
Then all regular files appear and the symlink is excluded
```

**Scenario 3: Canonicalized path outside root is rejected**

```gherkin
Given a story file whose canonicalized path is outside the canonicalized backlog root
When `read_story_file` resolves it
Then it returns an error and the story is skipped
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Security**     | The walk never follows symlinks; canonicalization is defense-in-depth |
| **Portability**  | `WalkDir::follow_links(false)` is the default but must be made explicit |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/repository.rs`
- **Suggested patterns:** `WalkDir::new(&backlog_root).follow_links(false)` and verify each canonicalized story path `starts_with(&canonicalized_backlog_root)` before parsing.
- **Testing approach:** Plant a symlink in a test fixture, assert it is excluded from `read_repository`.
- **Migration / backward compatibility:** Users who legitimately symlinked story directories will need real files; document this in the changelog.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `WalkDir` call explicitly disables link following
- [ ] Canonicalized story paths are verified against the canonicalized backlog root
- [ ] Test plants a symlink and asserts it is skipped
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-008: Path containment helper         | Story   | Draft     | Provides the shared containment check  |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Set `follow_links(false)` on the repository walk
- Add canonicalized-against-root check in `read_story_file`
- Add symlink-planting test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should the walk skip symlinked directories too, or only files?   | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
