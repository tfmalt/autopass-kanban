---
id: US-012
type: user-story
status: done
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-24T12:19:22+0200
work_done: 2026-06-24T16:05:31+0200
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T16:05:31+0200
---

# User Story: Atomic markdown writes via temp file and rename

---

## Story Statement

**As a** developer whose `kanban` command may be interrupted,
**I want** markdown writes to be atomic (temp file + fsync + rename),
**so that** a panic, SIGTERM, or disk-full mid-write cannot leave a story or
sprint file truncated or empty.

---

## Background

Every writer in `story.rs` (lines 105, 198, 307, 358, 410, 481), `sprint.rs`
(193, 262, 279, 842), `epic.rs:70`, `markdown.rs:259`, `doctor.rs` (21, 179,
213), `config.rs:572`, and `web-server/src/lib.rs:1456` uses `fs::write`
directly. AGENTS.md says the tool must be "safe for human-edited markdown files";
a partial write violates that. `tempfile` is already a workspace dependency.

**Complexity: medium** — introduce one atomic-write helper and replace each
`fs::write` call site; add a crash-simulation test.

---

## Acceptance Criteria

**Scenario 1: Interrupted write leaves original intact**

```gherkin
Given a story file exists with valid content
When a mutation is interrupted after the temp file is written but before rename
Then the original story file is unchanged and valid
And no partial content is visible at the final path
```

**Scenario 2: Successful write is durable**

```gherkin
Given a successful `kanban story move`
When the command returns
Then the target file has been fsynced and renamed into place
```

**Scenario 3: All writers use the atomic helper**

```gherkin
Given the codebase after the change
When grepping for direct `fs::write` in core and web-server writers
Then no writer bypasses the atomic-write helper
```

---

## Non-Functional Requirements

| Area                | Requirement                                                              |
| ------------------- | ------------------------------------------------------------------------ |
| **Data integrity**  | Writes are atomic on Unix (rename) and Windows (same-volume move)        |
| **Performance**     | The extra fsync cost is acceptable for a local CLI                        |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** all writers in `crates/core` and `crates/web-server/src/lib.rs`
- **Suggested patterns:** `pub(crate) fn atomic_write(path: &Path, contents: &str) -> Result<()>` using `tempfile::NamedTempFile::new_in(path.parent()?)`, write, `file.sync_all()`, then `persist(path)`. Promote `tempfile` from dev-dependency to a `core` runtime dependency.
- **Testing approach:** A test that writes a file, then calls `atomic_write` with a wrapper that simulates failure after temp-file creation, and asserts the original is intact.
- **Migration / backward compatibility:** No behavior change on success; only crash-safety improves.

### Estimation Rules

`story_points` is `5` (complexity: medium).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] `atomic_write` helper exists in `core` and replaces every `fs::write` in writers
- [ ] Temp file is created in the target's parent directory (same-volume rename)
- [ ] `fsync` is called before rename
- [ ] Test simulates mid-write failure and verifies original is intact
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| `tempfile` crate                        | Dependency | Available | Promote to core runtime dep            |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add `atomic_write` helper in `core` using `NamedTempFile::persist`
- Replace `fs::write` call sites across writers
- Add crash-simulation test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should fsync the parent directory too (for durability on rename)? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
