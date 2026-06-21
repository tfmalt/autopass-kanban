---
id: US-002
type: user-story
status: done
epic: EP-001
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-21T21:31:24+0200
work_done: 2026-06-21T21:34:36+0200
created: 2026-06-21T16:48:56+0200
updated: 2026-06-21T21:34:36+0200
activated: 2026-06-21T21:22:12+0200
---

# User Story: Discover and install kanban agent skills into the user's agent config

---

## Story Statement

**As a** developer who uses an AI assistant that discovers skills from a
user-level config directory (for example opencode's
`~/.config/opencode/skills/`),
**I want** the installer to scan my home directory for the most appropriate
agent skill install location, prompt me to confirm or override it, and then
install the `kanban-backlog-maintainer` and `kanban-developer` skills there,
**so that** those skills are available in every git repository I work in
without per-project copying, and so that I am in control of where on my disk
they live.

---

## Background

The repository ships two agent skills under `skills/`:

- `skills/kanban-backlog-maintainer/` — operates any markdown kanban backlog.
- `skills/kanban-developer/` — implements and maintains this Rust workspace.

Today they are only usable from inside this repository because
`.agents/skills/<name>` symlinks point at `../../skills/<name>`. AI assistants
that look in user-level config (opencode's `~/.config/opencode/skills/`,
`$XDG_CONFIG_HOME/opencode/skills/`, `$OPENCODE_HOME/skills/`) never see them.

Different users run different assistants, different assistants use different
discovery paths, and even one assistant's path varies by `$XDG_CONFIG_HOME` and
whether the user has already opted into a non-default config dir. Hard-coding
one path would be wrong. The installer must instead discover candidate
locations from the user's environment, present the best candidate, and let the
user confirm or override before writing anything.

---

## Acceptance Criteria

**Scenario 1: opencode user config already exists**

```gherkin
Given `~/.config/opencode/skills/` already exists on the user's machine
  and `$XDG_CONFIG_HOME` is unset
When the installer runs the skill-discovery step
Then it selects `~/.config/opencode/skills/` as the candidate skill directory
  and prompts the user: "Install kanban skills to ~/.config/opencode/skills/? [Y/n]"
  and on `Y` installs both `kanban-backlog-maintainer` and `kanban-developer` there
  and on `n` prompts for an alternative path and retries discovery against that path
```

**Scenario 2: `$XDG_CONFIG_HOME` is set**

```gherkin
Given `$XDG_CONFIG_HOME` is set to `/home/user/.cfg`
  and `/home/user/.cfg/opencode/skills/` does not yet exist
When the installer runs the skill-discovery step
Then it selects `/home/user/.cfg/opencode/skills/` as the candidate
  and prompts the user to confirm
  and creates the directory tree on confirmation before copying skills
```

**Scenario 3: `$OPENCODE_HOME` overrides everything**

```gherkin
Given `$OPENCODE_HOME` is set to `/srv/opencode`
When the installer runs the skill-discovery step
Then it selects `/srv/opencode/skills/` as the candidate skill directory
  (preferring `$OPENCODE_HOME/skills/` over `$XDG_CONFIG_HOME/opencode/skills/`
   and over `~/.config/opencode/skills/`)
```

**Scenario 4: No known agent config directory exists**

```gherkin
Given the user has no `opencode` config directory and no relevant env vars set
When the installer runs the skill-discovery step
Then it scans `~/.config/`, `~/.local/share/`, and `~/.config/opencode/`
  and reports to the user which candidate locations it checked and found nothing in
  and proposes `~/.config/opencode/skills/` as the default
  and prompts the user: "No agent config found. Install to ~/.config/opencode/skills/? [Y/n/path]"
  and on `Y` creates the directory tree and installs the skills
  and on a typed path, validates and uses that path
```

**Scenario 5: User passes `--skills-dir` to skip the prompt**

```gherkin
Given the user runs `sh scripts/install.sh --skills-dir ~/.my-agents/skills`
When the installer runs the skill-discovery step
Then it does not scan and does not prompt
  and installs both skills into `~/.my-agents/skills/`
  and creates the directory tree if it does not exist
```

**Scenario 6: User passes `--no-skills` to skip skill installation entirely**

```gherkin
Given the user runs `sh scripts/install.sh --no-skills`
When the installer runs
Then it does not scan for, prompt about, or install any agent skills
  and still installs the kanban binary and completions per US-001
  and prints a single-line notice that skill installation was skipped
```

**Scenario 7: Existing skill install is upgraded in place**

```gherkin
Given `~/.config/opencode/skills/kanban-developer/SKILL.md` already exists
  and its contents are an older version of the shipped skill
When the installer runs and the user confirms the same target directory
Then the installer overwrites `SKILL.md` and `plugin.json` for each kanban skill
  and prints a diff summary of what changed
  and does not touch any other skills in that directory
```

**Scenario 8: Skill installation is recorded in the install manifest**

```gherkin
Given the installer has just installed the two kanban skills
When the installer writes the install manifest
Then `<prefix>/lib/kanban/manifest.txt` contains one line per installed file
  (each `SKILL.md` and `plugin.json`)
  with the file path, SHA-256, source path inside the repo, and installer version
  so that US-004 can uninstall them precisely
```

**Scenario 9: Dry run previews the discovered path and every file copy**

```gherkin
Given the user runs `sh scripts/install.sh --dry-run`
When the installer runs the skill-discovery step
Then it prints the candidate skill directory it would use
  and prints every file it would copy
  and prompts for confirmation as it would in a real run
  and does not modify the filesystem regardless of the user's prompt answer
```

---

## Non-Functional Requirements

| Area                | Requirement                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------- |
| **Portability**     | Discovery uses POSIX `test -d` and `printenv` only; no bash-isms in the discovery path   |
| **Security**        | Never writes outside the confirmed skill directory; refuses paths containing `..` after expansion |
| **Traceability**    | Logs every discovery candidate it checked (hit or miss) to stderr; logs every file copy with `+ ` prefix |
| **Auditability**    | Every installed skill file is recorded in the manifest with its source hash              |
| **Performance**     | Discovery completes in under 200 ms on a warm home directory                             |
| **Backward compatibility** | Overwriting an existing skill install is intentional and reversible via US-004 |

---

## Technical Notes

- **Requirement refs:** `EP-001#scope` (agent skills install), `EP-001#acceptance-criteria` (skill discovery and prompt)
- **Component / Module:** `scripts/install.sh` skill-discovery and skill-copy sections; sources its files from `skills/kanban-backlog-maintainer/` and `skills/kanban-developer/` in the repository (or from a fetched release tarball in US-005).
- **Key integration points:** the installer copies `SKILL.md` and `plugin.json` for each skill. It does not copy the symlinked `.agents/skills/` directory — that is repository-local scaffolding, not the canonical source.
- **Suggested patterns:**
  - A `discover_skills_dir` function that returns the first hit in this priority order: `--skills-dir` flag, `$OPENCODE_HOME/skills/`, `$XDG_CONFIG_HOME/opencode/skills/`, `~/.config/opencode/skills/`, then a prompted default.
  - A `confirm_or_override` function that prints the candidate and reads `Y`, `n`, or a path from `/dev/tty` (so `curl | sh` in US-005 can still prompt).
  - A `copy_skill` function that takes a skill name and a target dir, copies `SKILL.md` and `plugin.json`, and appends one manifest line per file.
- **Data model hints:** Reuses the manifest format from US-001 (`<path>\t<sha256>\t<source>\t<installer-version>`).
- **Testing approach:**
  - Unit-test `discover_skills_dir` against a fixture `HOME` with each of the env var combinations above.
  - Integration-test the full install of both skills in a clean `HOME` fixture and assert the files exist and that the manifest lists them.
  - Assert that re-running upgrades the files and does not duplicate manifest entries.
- **Migration / backward compatibility:** If a user previously copied skills manually, the installer's overwrite-with-prompt flow must not silently destroy their changes — the diff summary in Scenario 7 is the audit hook.

### Estimation Rules

`story_points` is `5` (M): discovery plus prompt flow plus copy plus manifest integration is well-bounded, but the priority order of env vars and the confirm-or-override UX need care.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set when this story moves to `in-progress`.

---

## Definition of Done

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] `scripts/install.sh` discovers, prompts, and installs both kanban skills
- [ ] Code reviewed and approved via pull request (minimum 1 reviewer)
- [ ] Unit tests for `discover_skills_dir` cover all env var combinations in the scenarios
- [ ] Integration test in `tests/install/` covers a clean `HOME` and a re-run upgrade
- [ ] README and `HOWTO.md` document `--skills-dir`, `--no-skills`, and the discovery prompt
- [ ] Workspace version bumped in `Cargo.toml` per the SemVer scheme in `AGENTS.md`
- [ ] `cargo fmt --all --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Dependencies

| Dependency                              | Type           | Status    | Notes                                                            |
| --------------------------------------- | -------------- | --------- | ---------------------------------------------------------------- |
| US-001: Local binary install            | Story          | Draft     | US-002 layers skill install on top of the same `scripts/install.sh`; the manifest helper and rc-edit sentinel pattern come from US-001 |
| `skills/kanban-*` source directories    | Code           | Available | Already in the repository                                        |
| `plugin.json` version field per skill   | Code           | Available | Already maintained per skill                                     |

---

## Sprint Task Log Guidance

Expected tasks once activated into a sprint:

- Implement `discover_skills_dir` with the env var priority order
- Implement `confirm_or_override` reading from `/dev/tty`
- Implement `copy_skill` for `SKILL.md` and `plugin.json`
- Integrate skill install into the main install flow behind `--no-skills` gate
- Add unit tests for discovery across env var combinations
- Add integration test for clean install and upgrade-in-place
- Document `--skills-dir`, `--no-skills`, and discovery behaviour in README and HOWTO.md

---

## Notes and Open Questions

| #   | Question / Assumption                                                                    | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Do we support installing skills for multiple assistants in one run, or only one target?  | Tooling lead | 2026-06-27 | No       |
| 2   | Should the installer copy the symlinked `.agents/skills/` or only the canonical `skills/`?| Tooling lead | 2026-06-27 | No       |
| 3   | Should `--no-skills` be the default when stdin is not a TTY (non-interactive `curl \| sh`)? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
