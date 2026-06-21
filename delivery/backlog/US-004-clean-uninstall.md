---
id: US-004
type: user-story
status: draft
epic: EP-001
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 3
work_started:
work_done:
created: 2026-06-21T16:48:56+0200
updated: 2026-06-21T16:48:56+0200
---

# User Story: Clean uninstall of the kanban binary, completions, skills, and rc edits

---

## Story Statement

**As a** developer who no longer wants `kanban` on my machine,
**I want** to run a single `scripts/uninstall.sh` script that removes exactly
the files the installer placed and reverts exactly the shell rc edits the
installer made,
**so that** my home directory is back in its pre-install state (modulo files I
edited myself) with no leftover `kanban` binary, completion, skill, or
sentinel-commented rc line.

---

## Background

US-001 through US-003 write a small, well-defined set of files and rc lines,
all recorded in `<prefix>/lib/kanban/manifest.txt` and tagged in shell rc files
with `# kanban-installer:` sentinels. That record is what makes a clean
uninstall possible: the uninstaller does not need to guess what to remove, it
reads the manifest and the sentinel-tagged rc lines and reverses them.

The risk to avoid is the inverse of the install risk: removing files the user
created themselves (a hand-edited `SKILL.md`, a `~/.local/bin/kanban` symlink
the user added for a different purpose). The uninstaller must only remove files
whose hash matches the manifest, and must only remove rc lines tagged with the
installer sentinel.

---

## Acceptance Criteria

**Scenario 1: Full uninstall after a clean install**

```gherkin
Given a working kanban install with a manifest at `<prefix>/lib/kanban/manifest.txt`
  and the user has not manually edited any installed file
When the user runs `sh scripts/uninstall.sh`
Then the uninstaller:
  - removes `~/.local/bin/kanban`
  - removes the bash completion file (if installed)
  - removes the zsh completion file (if installed)
  - removes both kanban skill directories from the discovered skills dir
  - removes the sentinel-commented `PATH` line from `.bashrc` / `.zshrc` / `.profile`
  - removes the sentinel-commented `fpath` and `compinit` lines from `.zshrc` (if added)
  - removes `<prefix>/lib/kanban/` (including `manifest.txt`)
  - leaves any file whose hash does not match the manifest in place with a warning
  and a new login shell can no longer find `kanban`
```

**Scenario 2: Uninstall with a custom prefix**

```gherkin
Given the user installed with `--prefix ~/bin`
When the user runs `sh scripts/uninstall.sh --prefix ~/bin`
Then the uninstaller reads `~/lib/kanban/manifest.txt`
  and removes only the files listed there under `~/bin/` and `~/lib/kanban/`
  and leaves any install at the default `~/.local/` untouched
```

**Scenario 3: Uninstall skips files the user edited**

```gherkin
Given the user edited `~/.config/opencode/skills/kanban-developer/SKILL.md`
  and the on-disk hash no longer matches the manifest hash
When the user runs `scripts/uninstall.sh`
Then the uninstaller:
  - prints "Skipping <path>: modified since install (hash mismatch)"
  - leaves the file on disk
  - continues removing every other matching file
  - exits with status 0 and a final notice listing the skipped files
```

**Scenario 4: Uninstall refuses to run without a manifest**

```gherkin
Given there is no `<prefix>/lib/kanban/manifest.txt`
When the user runs `scripts/uninstall.sh`
Then the uninstaller:
  - prints "No kanban install manifest found at <path>. Nothing to uninstall."
  - exits with status 0
  - does not search the filesystem for kanban files to remove
```

**Scenario 5: Uninstall removes only the sentinel-tagged rc lines**

```gherkin
Given the user's `.bashrc` contains:
  - a manually-added `export PATH="$HOME/bin:$PATH"` line (no sentinel)
  - the installer-added `export PATH="$HOME/.local/bin:$PATH" # kanban-installer: path` line
When the uninstaller runs
Then it removes only the sentinel-tagged line
  - and leaves the manually-added line untouched
  - and writes the resulting `.bashrc` via a temp file and atomic `mv` so a crash mid-edit cannot corrupt the rc file
```

**Scenario 6: Uninstall prompts before removing agent skills**

```gherkin
Given the manifest lists installed kanban skills under `~/.config/opencode/skills/`
When the uninstaller runs
Then it prompts: "Remove kanban skills from ~/.config/opencode/skills/? [Y/n]"
  - on `Y`, removes both `kanban-backlog-maintainer/` and `kanban-developer/`
  - on `n`, leaves the skills in place and prints a notice showing the paths
    so the user can remove them manually
  - in either case, continues removing the binary, completions, and rc edits
```

**Scenario 7: Dry-run previews every removal and rc edit**

```gherkin
Given the user runs `sh scripts/uninstall.sh --dry-run`
When the uninstaller runs
Then it prints:
  - every file it would remove, with the manifest hash and the on-disk hash
  - every rc line it would remove, with the file path
  - any files it would skip due to hash mismatch
  and does not modify the filesystem
  and exits with status 0
```

**Scenario 8: Uninstall after a partial re-install (US-003 interrupted)**

```gherkin
Given a previous upgrade was interrupted and the manifest reflects the new install
  but some old files from the previous install are still on disk and not in the manifest
When the uninstaller runs
Then it removes only the files listed in the manifest
  - and prints a notice listing the orphaned files it did not remove
    (because they are not in the manifest and might be user files)
  - and suggests the user inspect them manually
```

---

## Non-Functional Requirements

| Area                | Requirement                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------- |
| **Portability**     | Same POSIX `sh` constraints as US-001; no `sudo`                                          |
| **Security**        | Refuses to follow symlinks when removing; refuses to remove files outside the prefix or skills dir after path expansion |
| **Traceability**    | Logs every removal with `- ` prefix, every skip with `! ` prefix, every rc edit with `~ ` prefix |
| **Auditability**    | After completion, prints a summary listing files removed, files skipped, rc files edited, and the manifest's final state (deleted) |
| **Performance**     | Uninstall completes in under 1 second on a warm machine                                   |
| **Backward compatibility** | Tolerates a partially-corrupt manifest by removing the entries it can parse and warning about the rest |

---

## Technical Notes

- **Requirement refs:** `EP-001#acceptance-criteria` (clean uninstall), US-001 manifest, US-002 skill manifest, US-003 reconcile/orphan handling
- **Component / Module:** `scripts/uninstall.sh` (new); reads `<prefix>/lib/kanban/manifest.txt` written by US-001 through US-003.
- **Key integration points:** consumes the manifest format and the `# kanban-installer:` sentinel convention introduced in US-001.
- **Suggested patterns:**
  - A `remove_if_hash_matches` function that takes a path and an expected hash, skips with a warning on mismatch, and uses `rm -f` on match.
  - A `strip_sentinel_lines` function that takes an rc file path, reads it into a temp file omitting lines tagged `# kanban-installer:`, and atomically `mv`s the temp file back.
  - A `prompt_yes_no` function shared with US-002 for the skill-removal confirmation.
- **Data model hints:** No new manifest fields; the uninstaller is a pure consumer.
- **Testing approach:**
  - Integration-test: install via US-001+US-002 in a fixture `HOME`, then run `uninstall.sh`, then assert the `HOME` tree matches a pre-install snapshot (modulo the user's own files).
  - Integration-test the hash-mismatch skip and the sentinel-only rc edit.
  - Integration-test the no-manifest case.
- **Migration / backward compatibility:** If the manifest predates a field the uninstaller expects (for example a future `source` field), the uninstaller ignores the missing field rather than failing.

### Estimation Rules

`story_points` is `3` (S): the uninstaller is a pure consumer of the manifest and sentinel conventions established earlier, so the surface is small.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set when this story moves to `in-progress`.

---

## Definition of Done

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] `scripts/uninstall.sh` exists, is executable, and starts with `#!/bin/sh`
- [ ] After uninstall, a new shell cannot find `kanban` and no sentinel-tagged rc lines remain
- [ ] Files modified by the user are skipped, not removed
- [ ] Code reviewed and approved via pull request (minimum 1 reviewer)
- [ ] Integration test in `tests/install/` covers full uninstall, custom prefix, hash-mismatch skip, no-manifest, and dry-run
- [ ] README and `HOWTO.md` document `sh scripts/uninstall.sh` and its flags
- [ ] Workspace version bumped in `Cargo.toml` per the SemVer scheme in `AGENTS.md`
- [ ] `cargo fmt --all --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Dependencies

| Dependency                          | Type   | Status    | Notes                                                              |
| ----------------------------------- | ------ | --------- | ------------------------------------------------------------------ |
| US-001: Local binary install        | Story  | Draft     | Provides the manifest and rc sentinel convention the uninstaller consumes |
| US-002: Agent skills install        | Story  | Draft     | Provides the skill manifest entries the uninstaller consumes        |
| US-003: Idempotent upgrade          | Story  | Draft     | Provides the orphan-aware manifest the uninstaller must consume    |

---

## Sprint Task Log Guidance

Expected tasks once activated into a sprint:

- Implement `remove_if_hash_matches`
- Implement `strip_sentinel_lines` with atomic rc file rewrite
- Implement skill-removal prompt reusing US-002's `prompt_yes_no`
- Implement no-manifest short-circuit
- Implement dry-run flag reusing the same pattern as US-001
- Add integration tests for all eight scenarios
- Document `scripts/uninstall.sh` in README and HOWTO.md

---

## Notes and Open Questions

| #   | Question / Assumption                                                                | Owner        | Due        | Resolved |
| --- | ------------------------------------------------------------------------------------ | ------------ | ---------- | -------- |
| 1   | Should `uninstall.sh` also remove the `~/.config/kanban/` runtime dir used by `kanban web`? | Tooling lead | 2026-06-27 | No       |
| 2   | Should `uninstall.sh` accept a `--force` flag that ignores hash mismatches?         | Tooling lead | 2026-06-27 | No       |
| 3   | Should the sentinel comment include the installer version for traceability?         | Tooling lead | 2026-06-27 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
