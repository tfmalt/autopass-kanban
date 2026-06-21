---
id: US-003
type: user-story
status: draft
epic: EP-001
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started:
work_done:
created: 2026-06-21T16:48:56+0200
updated: 2026-06-21T16:48:56+0200
---

# User Story: Idempotent upgrade and reinstall of an existing kanban install

---

## Story Statement

**As a** developer who already has `kanban` installed via `scripts/install.sh`,
**I want** to re-run the same installer to upgrade the binary, completions, and
agent skills in place,
**so that** I always end up with the latest pinned version with no manual
cleanup, no duplicate shell rc edits, and no leftover partial files from the
previous install.

---

## Background

US-001 and US-002 establish the first-install flow. Users will re-run the
installer for three reasons: to pick up a new release of `kanban`, to pick up
updated agent skills, and to repair a broken install. All three must feel like
the same operation: run the script, get the new state, end up clean.

The two failure modes that make "re-run the installer" scary are duplicate rc
edits (every re-run appends another `export PATH=...` line to `.bashrc`) and
orphaned files (a skill file renamed upstream stays on disk forever). This
story makes re-runs idempotent by relying on the install manifest written by
US-001 and US-002, the sentinel-commented rc edits introduced in US-001, and a
small pre-install reconciliation step that removes files the new install no
longer ships.

---

## Acceptance Criteria

**Scenario 1: Re-run with the same prefix and skills-dir upgrades in place**

```gherkin
Given the user has a working kanban install recorded in `<prefix>/lib/kanban/manifest.txt`
  and a newer prebuilt `kanban` binary is supplied via `--binary`
When the user re-runs `sh scripts/install.sh --binary ./target/release/kanban`
Then the installer:
  - reads the existing manifest
  - overwrites `~/.local/bin/kanban` with the new binary
  - overwrites each completion file with the new `kanban completion` output
  - overwrites each skill `SKILL.md` and `plugin.json` with the new copies
  - appends no new rc lines (the sentinel is already present)
  - rewrites `manifest.txt` with the new hashes and installer version
  and a new shell runs `kanban --version` and reports the new version
```

**Scenario 2: Re-run detects and removes orphaned files**

```gherkin
Given the previous manifest lists a file `<prefix>/share/zsh/site-functions/_kanban`
  and the new install will write zsh completions to `~/.zsh/completions/_kanban` instead
When the user re-runs the installer
Then the installer:
  - diffs the previous manifest against the list of files the new install will write
  - removes every file that was previously recorded but is no longer written
  - logs each removal with a `- ` prefix to stderr
  - updates `manifest.txt` so it reflects only the current install
  and does not remove files that were not recorded in the previous manifest
```

**Scenario 3: Re-run with a different `--prefix` is treated as a fresh install**

```gherkin
Given the previous install used `~/.local/bin` as the prefix
  and the user re-runs with `--prefix ~/bin`
When the installer runs
Then it does not attempt to reconcile against the previous manifest at `~/.local/lib/kanban/manifest.txt`
  - it performs a fresh install at `~/bin`
  - it writes a new manifest at `~/lib/kanban/manifest.txt`
  - it prints a notice that the previous install at `~/.local/bin` was left untouched
    and points the user at `scripts/uninstall.sh --prefix ~/.local` (US-004) to remove it
```

**Scenario 4: Re-run with a different `--skills-dir` reconciles the previous skill install**

```gherkin
Given the previous install placed skills in `~/.config/opencode/skills/`
  and the user re-runs with `--skills-dir ~/.my-agents/skills`
When the installer runs
Then it:
  - installs the skills into `~/.my-agents/skills/`
  - removes the previously installed kanban skill files from `~/.config/opencode/skills/`
    (only the files listed in the previous manifest, never other skills the user added)
  - updates the manifest to point at the new location
  and prints a notice listing the files it removed from the previous skills dir
```

**Scenario 5: Re-run refuses to downgrade silently**

```gherkin
Given the installed binary reports version `26.7.0101`
  and the supplied `--binary` reports version `26.6.2107`
When the user re-runs the installer
Then the installer:
  - detects the downgrade by comparing `kanban --version` output before and after the candidate binary is staged
  - prompts the user: "Downgrade from 26.7.0101 to 26.6.2107? [y/N]"
  - on `N` (the default), exits 0 without modifying the install
  - on `y`, proceeds with the downgrade and notes it in the manifest
```

**Scenario 6: Interrupted re-run leaves the previous install intact**

```gherkin
Given the installer is interrupted by `SIGINT` partway through overwriting files
When the user inspects the install afterwards
Then the binary at `<prefix>/bin/kanban` is either the previous version or the new version (never a half-written file)
  - because the installer stages the new binary at a temp path and `mv`'s it into place atomically
  - and `manifest.txt` is only rewritten after every file copy has succeeded
  - so a previous install remains usable until the new install is fully written
```

**Scenario 7: Re-run after a manual edit to an installed skill file**

```gherkin
Given the user manually edited `~/.config/opencode/skills/kanban-developer/SKILL.md`
  and re-runs the installer with a newer skill source
When the installer is about to overwrite that file
Then it:
  - detects the local edit by comparing the on-disk hash against the previous manifest hash
  - prompts the user: "Local edits to <path> will be overwritten. Continue? [y/N]"
  - on `N`, skips that file, leaves the user's edit in place, and notes the skip in the manifest
  - on `y`, overwrites and records the new hash
```

**Scenario 8: Dry-run shows the upgrade diff**

```gherkin
Given the user runs `sh scripts/install.sh --binary <new> --dry-run`
  and a previous manifest exists
When the installer runs
Then it prints:
  - every file that would be overwritten, with old and new SHA-256
  - every file that would be removed (orphaned from the previous install)
  - every file that would be added (new in this install)
  - the rc edits that would be made (none, if sentinels are present)
  and does not modify the filesystem
```

---

## Non-Functional Requirements

| Area                | Requirement                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------- |
| **Portability**     | Atomic file replacement uses `mv` across the same filesystem; never cross-filesystem `mv` without copy+fsync |
| **Security**        | Temp files are created with `mktemp` in the same directory as the target and `umask 077`; never world-writable |
| **Traceability**    | Every overwrite, removal, and skip is logged to stderr with `~ `, `- `, or `! ` prefixes respectively |
| **Auditability**    | `manifest.txt` records the installer version and timestamp of the last install as header lines |
| **Performance**     | Re-run with no changes (same binary, same skills) completes in under 1 second and writes nothing |
| **Backward compatibility** | Tolerates a missing or malformed manifest by treating the run as a fresh install with a warning |

---

## Technical Notes

- **Requirement refs:** `EP-001#acceptance-criteria` (idempotent re-run), US-001 manifest format, US-002 skill install
- **Component / Module:** `scripts/install.sh` upgrade path; reads and rewrites `<prefix>/lib/kanban/manifest.txt`.
- **Key integration points:** consumes the manifest written by US-001 and US-002; produces a manifest that US-004 consumes for uninstall.
- **Suggested patterns:**
  - A `reconcile_manifest` function that takes the previous manifest and the planned file list and returns three lists: `to_overwrite`, `to_remove`, `to_add`.
  - Atomic binary replacement: stage at `<prefix>/bin/.kanban.new.<pid>`, `chmod +x`, `mv -f` over the target. Same pattern for `manifest.txt`.
  - Rc edit idempotency: always check for the sentinel comment before appending.
- **Data model hints:** The manifest header gains `# installer-version: <semver>` and `# installed-at: <iso8601>` lines; the rest of the format is unchanged from US-001.
- **Testing approach:**
  - Unit-test `reconcile_manifest` against synthetic previous/planned lists.
  - Integration-test: install v1, then install v2 in the same fixture `HOME`, assert the final state and the manifest.
  - Integration-test the `SIGINT` case with `kill -INT` mid-run in CI.
- **Migration / backward compatibility:** A pre-existing 0.x install without a manifest is treated as a fresh install with a printed warning, never as an error.

### Estimation Rules

`story_points` is `5` (M): reconcile logic, atomic replacement, downgrade prompt, and interrupted-run safety are each small but add up.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set when this story moves to `in-progress`.

---

## Definition of Done

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] Re-running the installer produces no duplicate rc edits and no orphaned files
- [ ] Atomic file replacement is verified by a `SIGINT`-mid-run integration test
- [ ] Code reviewed and approved via pull request (minimum 1 reviewer)
- [ ] Unit tests for `reconcile_manifest` pass on macOS and Linux CI
- [ ] Integration test in `tests/install/` covers upgrade, orphan removal, and downgrade refusal
- [ ] README and `HOWTO.md` document that re-running the installer is the upgrade path
- [ ] Workspace version bumped in `Cargo.toml` per the SemVer scheme in `AGENTS.md`
- [ ] `cargo fmt --all --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Dependencies

| Dependency                          | Type   | Status    | Notes                                                              |
| ----------------------------------- | ------ | --------- | ------------------------------------------------------------------ |
| US-001: Local binary install        | Story  | Draft     | Provides the manifest, sentinel rc edits, and atomic-replace hooks |
| US-002: Agent skills install        | Story  | Draft     | Provides the skill manifest entries that upgrade must reconcile    |

---

## Sprint Task Log Guidance

Expected tasks once activated into a sprint:

- Implement `reconcile_manifest` with overwrite/remove/add lists
- Implement atomic binary and manifest replacement
- Implement downgrade detection and prompt
- Implement local-edit detection and prompt for skill files
- Add rc-edit sentinel idempotency check before appending
- Add unit tests for `reconcile_manifest`
- Add integration tests for upgrade, orphan removal, downgrade refusal, and `SIGINT` mid-run
- Document the upgrade flow in README and HOWTO.md

---

## Notes and Open Questions

| #   | Question / Assumption                                                                | Owner        | Due        | Resolved |
| --- | ------------------------------------------------------------------------------------ | ------------ | ---------- | -------- |
| 1   | Should downgrade refusal be bypassable with a `--force` flag?                        | Tooling lead | 2026-06-27 | No       |
| 2   | Should the installer back up overwritten files to `<prefix>/lib/kanban/backup/`?     | Tooling lead | 2026-06-27 | No       |
| 3   | Is the `SIGINT` safety requirement in scope for this Epic, or a follow-up?           | Tooling lead | 2026-06-27 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
