---
id: US-032
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

# User Story: Secure tempfile for the uninstall script

---

## Story Statement

**As a** user running `kanban uninstall`,
**I want** the temp install script written with `O_EXCL` and `0o700`
permissions outside world-writable `/tmp`,
**so that** a local attacker cannot swap the script before `sh` executes it.

---

## Background

`crates/cli/src/self_manage.rs:199-211` writes
`{prefix}-{pid}-{nanos}.sh` to `std::env::temp_dir()` with `fs::write` (no
`O_EXCL`, no `0o600`), then `sh`s it. On shared hosts a local attacker can win
the race. The `unwrap_or_default()` can yield a predictable name if the clock
is pre-EPOCH.

**Complexity: low** — use `tempfile::Builder::new().permissions(0o700)` or write
under `$XDG_RUNTIME_DIR`.

---

## Acceptance Criteria

**Scenario 1: Temp script is exclusive and private**

```gherkin
Given `kanban uninstall` writes its temp script
When the file is created
Then it uses `O_EXCL` and mode `0o700`
And it lives outside world-writable `/tmp` when `XDG_RUNTIME_DIR` is set
```

**Scenario 2: Race cannot swap the script**

```gherkin
Given an attacker pre-creates the predicted temp path
When `kanban uninstall` writes the script
Then creation fails safely rather than overwriting the attacker's file
```

---

## Non-Functional Requirements

| Area             | Requirement                                                          |
| ---------------- | -------------------------------------------------------------------- |
| **Security**     | No predictable-path temp-file race for executed scripts              |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/self_manage.rs`
- **Suggested patterns:** `tempfile::Builder::new().permissions(Permissions::from_mode(0o700)).tempfile_in(xdg_runtime_dir_or_cache())` then write + `sh`.

### Estimation Rules

`story_points` is `3` (complexity: low).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Temp script uses `O_EXCL` + `0o700`
- [ ] Prefers `$XDG_RUNTIME_DIR`/cache over `/tmp`
- [ ] Test covers pre-existing-path collision
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

- Replace `fs::write` with `tempfile::Builder` exclusive create
- Prefer `XDG_RUNTIME_DIR`/cache dir
- Add collision test

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
