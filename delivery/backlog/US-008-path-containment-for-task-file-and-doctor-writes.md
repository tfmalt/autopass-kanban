---
id: US-008
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 8
work_started: 2026-06-24T11:17:15+0200
work_done: 2026-06-24T11:17:20+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T11:17:20+0200
---

# User Story: Path containment for task_file frontmatter and doctor writes

---

## Story Statement

**As a** developer maintaining a backlog with `kanban`,
**I want** every write derived from `task_file` frontmatter or doctor `file_path`
to stay inside the canonicalized backlog root,
**so that** a malicious or malformed `task_file: ../../etc/cron.d/x` value or a
symlinked tree cannot cause `kanban` to write outside the repository.

---

## Background

`crates/core/src/doctor.rs:176` joins the `task_file` frontmatter value verbatim
to the story's parent, and `doctor.rs:88-92` writes to
`repo_root.join(file_path)` where `file_path` can be an absolute path when
`relative_path`'s `strip_prefix` falls back. `validate.rs` has no rule for
`task_file` shape. A `task_file` containing `..` or an absolute path causes
writes outside the backlog root; the read side (`repository.rs:173`) will load
any file the OS user can read.

**Complexity: high** — touches the write path, the read path, validation, and
requires a shared containment helper plus symlink/`..` tests.

---

## Acceptance Criteria

**Scenario 1: Unsafe task_file is rejected on read and validate**

```gherkin
Given a story whose frontmatter contains `task_file: ../../../etc/passwd`
When `kanban validate .` runs
Then it reports an `invalid-task-file-path` issue
And `kanban story show <id>` does not attempt to read outside the backlog root
```

**Scenario 2: Doctor fix refuses to write outside the backlog root**

```gherkin
Given a doctor issue whose `file_path` resolves outside the canonicalized backlog root
When `kanban doctor fix` applies the fix
Then it returns an error naming the out-of-tree path
And no file outside the backlog root is created or modified
```

**Scenario 3: Containment helper is shared by all writers**

```gherkin
Given any writer in core, doctor, or web-server that joins a user-derived path to the repo root
When the resolved path is computed
Then it is canonicalized and checked with `starts_with(canonicalized_repo_root)`
And a mismatch yields an error rather than a write
```

**Scenario 4: Symlinked task_file cannot escape**

```gherkin
Given a `task_file` value that resolves through a symlink to a path outside the backlog root
When the writer resolves the path
Then containment check fails and no write occurs
```

---

## Non-Functional Requirements

| Area             | Requirement                                                                   |
| ---------------- | ----------------------------------------------------------------------------- |
| **Security**     | No `fs::write`/`fs::remove_file`/`fs::rename` reaches a path outside the root |
| **Traceability** | Validation issues cite the offending field and resolved path                  |
| **Portability**  | Containment uses `std::fs::canonicalize` which resolves symlinks on all OSes  |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/core/src/repository.rs`, `crates/core/src/doctor.rs`,
  `crates/core/src/validate.rs`, plus writers in `story.rs`/`sprint.rs`/`epic.rs`/`markdown.rs`
- **Suggested patterns:** Add `pub(crate) fn ensure_path_inside(repo_root: &Path, resolved: &Path) -> Result<PathBuf>` in `util.rs` that canonicalizes both and verifies `starts_with`; call it before every write involving user-derived paths.
- **Data model hints:** Add a `validate.rs` rule `invalid-task-file-path` rejecting `..`, path separators, and absolute `task_file` values.
- **Testing approach:** Unit tests with a `task_file: ../../evil` fixture and a symlinked story; assert the writer bails and no out-of-tree file appears.
- **Migration / backward compatibility:** Existing safe `task_file` values continue to work; only unsafe shapes are newly rejected.

### Estimation Rules

`story_points` is `8` (complexity: high) — the helper is small but it must be
threaded through every writer and validator, with new tests for traversal and
symlink cases.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `ensure_path_inside` helper exists in `core` and is called before every write involving user-derived paths
- [ ] `validate.rs` emits `invalid-task-file-path` for `..`, separator, or absolute values
- [ ] Tests cover `..` traversal, absolute path, and symlink escape
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| US-009: Disable symlink following       | Story   | Draft     | Related; implement together or sequence |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add `ensure_path_inside` helper with canonicalize + `starts_with`
- Thread it through `doctor.rs`, `story.rs`, `sprint.rs`, `epic.rs`, `markdown.rs` writers
- Add `invalid-task-file-path` validate rule
- Add traversal and symlink tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should containment also bound reads, or only writes?             | Tooling lead | 2026-07-04 | Yes — writes only per the written ACs; reads stay bounded by OS permissions and the US-009 symlink-following change. `validate` rejects unsafe `task_file` shapes so unsafe reads never happen for stories that pass validation. |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
