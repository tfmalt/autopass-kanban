---
id: EP-001
type: epic
status: done
phase: 1
owner: Thomas Malt / Tooling Lead
milestone: MP1
created: 2026-06-21T16:48:56+0200
updated: 2026-06-21T16:48:56+0200
---

# Epic: Self-contained local install of the kanban tool and its agent skills

---

---

## Business Context

The kanban CLI is currently only usable by developers who clone this repository,
install the Rust toolchain, and run `cargo build`. There is no path from "I heard
about this tool" to "`kanban` works in my shell" that does not require a working
Rust development environment and intimate knowledge of the workspace layout.

The two agent skills shipped with the tool (`kanban-backlog-maintainer` and
`kanban-developer`) are likewise locked inside the repository. AI assistants
that support user-level skill discovery (for example opencode's
`~/.config/opencode/skills/`) cannot pick them up without manual copying, so
the value of the skills is bounded by the value of the tool itself.

This Epic introduces a self-contained installer that places the prebuilt
`kanban` binary and the agent skills into standard user-local locations on
macOS and Linux without `sudo`, plus a remote `curl | sh` flow that lets a new
user install directly from the git repository without cloning it.

---

## Business Value

- **Primary benefit:** A new user can install `kanban` and its agent skills on
  any macOS or Linux machine in under a minute, with no Rust toolchain and no
  `sudo`, by running one command fetched from the git repository.
- **Secondary benefit:** Existing contributors can re-run the same installer to
  upgrade or repair their local install, and can uninstall cleanly when they
  stop using the tool.
- **Risk if not done:** The tool stays gated behind a build step, adoption stays
  limited to the maintainers, and the agent skills never reach the AI
  assistants that would benefit from them.

---

## Users and Stakeholders

| Role                        | Involvement                                                        |
| --------------------------- | ------------------------------------------------------------------ |
| New user / developer        | Runs the installer on their machine; wants `kanban` on PATH fast   |
| Existing contributor        | Re-runs the installer to upgrade; expects idempotent behaviour     |
| AI assistant (opencode etc) | Discovers installed agent skills in user-level config directories  |
| Tooling lead                | Owns release artifacts, checksums, and the install script contents |
| Release pipeline (CI)       | Publishes versioned release artifacts consumed by the installer    |

---

## Scope

### In Scope

- A POSIX `sh` install script committed to the repository at
  `scripts/install.sh` that, with no arguments, installs the prebuilt `kanban`
  binary and the agent skills into standard user-local locations on macOS and
  Linux.
- Discovery of a suitable install prefix and a suitable agent skill install
  directory by scanning the user's environment and existing config, then
  prompting the user to confirm or override the discovered location.
- PATH bootstrap: appending `~/.local/bin` (or the discovered equivalent) to
  the user's shell rc file only when it is not already present.
- Shell completion installation for bash and zsh via `kanban completion`.
- Idempotent re-runs that upgrade the binary and skills in place without
  duplicate rc edits or leftover partial installs.
- A matching `scripts/uninstall.sh` that removes exactly what the installer
  placed and reverts rc edits the installer made.
- A `curl | sh` remote entry point that fetches `scripts/install.sh` from a
  pinned git ref and executes it, with explicit version selection and a
  documented escape hatch (`--version`, `--prefix`, `--dry-run`).
- Versioned, checksummed release artifacts published from CI so the installer
  can verify integrity before writing anything to the user's home directory.
- Updating the workspace version in `Cargo.toml` per the SemVer scheme in
  `AGENTS.md` as installer work lands.

### Out of Scope

- Windows native support (PowerShell installer, `scoop`, `winget`). A future
  Epic can pick this up; the POSIX script must not be designed in a way that
  blocks it.
- System-wide `/usr/local/bin` installs requiring `sudo`. The installer is
  user-local only; package managers (Homebrew formula, AUR, Debian package) are
  a separate Epic.
- A first-class graphical installer or TUI. The installer is a script with
  optional flags and a small number of yes/no prompts.
- Auto-update / background update daemon. Upgrades are explicit re-runs of the
  installer.
- Signing release artifacts with a PGP key or Sigstore. Checksum verification
  against a checksums file committed alongside the release is in scope; deeper
  signing is a follow-up.
- Installing the kanban agent skills into project-local `.opencode/agents/`
  directories. The installer targets user-level discovery only; project-local
  wiring stays manual for now.

---

## Acceptance Criteria

- [ ] On a clean macOS user account, running `scripts/install.sh` with no
      arguments results in `kanban --version` succeeding in a freshly opened
      shell with no manual PATH editing by the user.
- [ ] On a clean Linux user account (Debian and Alpine), the same
      `scripts/install.sh` invocation succeeds and `kanban --version` works in a
      freshly opened shell.
- [ ] The installer discovers a candidate agent skill install directory by
      scanning the user's home directory and environment (for example
      `~/.config/opencode/skills/`, `~/.local/share/opencode/skills/`, or an
      existing `opencode` config), prompts the user to confirm or override, and
      installs both `kanban-backlog-maintainer` and `kanban-developer` there.
- [ ] Running `scripts/install.sh` a second time upgrades the binary and skills
      in place without duplicating any shell rc edits and without leaving
      partial files behind on failure.
- [ ] Running `scripts/uninstall.sh` removes every file the installer placed
      (binary, completions, skills) and reverts every shell rc edit the
      installer made, leaving the user's home directory in its pre-install
      state modulo files the user edited themselves.
- [ ] `curl -fsSL <repo>/scripts/install.sh | sh` installs the same artifacts
      the local `scripts/install.sh` would install for the matching git ref,
      and supports `--version <tag>` to install a specific release.
- [ ] The installer refuses to write the binary if the downloaded artifact's
      SHA-256 does not match the checksum recorded in the matching release
      checksums file, and exits non-zero with a clear message.
- [ ] `kanban completion bash` and `kanban completion zsh` output is installed
      by the installer into the conventional completion directories for the
      detected shell, and completions work in a freshly opened shell.
- [ ] The README documents the local and remote install commands, the supported
      flags, and the standard install locations.

---

## Non-Functional Requirements

| Area                 | Requirement                                                                                  |
| -------------------- | -------------------------------------------------------------------------------------------- |
| **Performance**      | Installer completes a fresh install in under 10 seconds on a warm machine with cached artifacts |
| **Portability**      | Installer runs on macOS and Linux with `/bin/sh` (POSIX), no GNU-isms required for the core flow |
| **Security**         | No `sudo`; no executing downloaded code without checksum verification; refuse silent fallbacks |
| **Traceability**     | Installer logs every file it writes and every rc edit it makes; `--dry-run` previews the same |
| **Auditability**     | Installer writes a manifest at `<prefix>/lib/kanban/manifest.txt` listing installed files and versions |
| **Observability**    | Installer exits non-zero on any failure with a single-line cause; `--verbose` adds per-step detail |
| **Backward compatibility** | Re-running the installer over a previous 0.x install upgrades without manual cleanup |

---

## Architecture Considerations

- **Relevant architecture principles:** Markdown-first source of truth; the
  installer never depends on a database, generated state store, or hidden
  metadata cache. The install manifest at `<prefix>/lib/kanban/manifest.txt` is
  a plain text file listing files and versions, owned by the installer, and not
  consumed by `kanban` itself at runtime.
- **Key patterns in play:**
  - POSIX `sh` script with explicit `set -eu` and a single sourced helper
    section, so the same script works for local and `curl | sh` execution.
  - Standard XDG-ish user-local layout: binary at `~/.local/bin/kanban`,
    completions at `~/.local/share/bash-completion/completions/` and
    `~/.zsh/completions/` (or the dirs `bash-completion` and zsh `$fpath`
    already use on the user's machine), skills under the discovered agent
    config directory.
  - Discovery-then-prompt pattern: scan for existing `opencode` config
    (`~/.config/opencode/`, `$XDG_CONFIG_HOME/opencode/`, `$OPENCODE_HOME`)
    and any existing kanban skill install before deciding where to place
    skills; the user confirms or overrides the discovered path.
- **ADR references:** None yet. If we adopt a release artifact naming scheme or
  a signing key, an ADR should record the decision.
- **Known risks or constraints:**
  - Users with non-standard shells (fish, nushell) will not get completion
    installation in this Epic; the installer must detect and skip gracefully
    with a clear message, not fail the install.
  - `curl | sh` has supply-chain implications; the checksum file must live
    alongside the binary in the release, and the installer must verify it
    before exec'ing any downloaded code.
  - The install script lives in `scripts/` which is already used by
    `scripts/wbs_report.py`; keep installer-only helpers under
    `scripts/install/` if the single-file script grows past ~400 lines.

The detailed implementation plan, file layout, environment variables, prompt
flow, and `curl | sh` request graph live in
`SPEC-installability.md` next to this Epic.

---

## Dependencies

| Dependency                       | Type           | Status    | Notes                                                                |
| -------------------------------- | -------------- | --------- | -------------------------------------------------------------------- |
| Prebuilt release artifacts       | Infrastructure | Pending   | CI must publish `kanban-<version>-<target>.tar.gz` + checksums file  |
| `kanban completion` subcommand   | Tool           | Available | Already shipped; installer consumes its stdout                       |
| opencode skill discovery layout  | External       | Confirmed | `~/.config/opencode/skills/<name>/SKILL.md` is the user-level layout |
| POSIX `sh` on macOS and Linux    | Platform       | Available | `/bin/sh` on macOS is bash; Alpine uses busybox ash                  |

---

## Child User Stories

| Story ID | Title                                                                         | Status  | Points |
| -------- | ----------------------------------------------------------------------------- | ------- | ------ |
| US-001   | Local install of the kanban binary with PATH and completion bootstrap         | Draft   | 8      |
| US-002   | Discover and install kanban agent skills into the user's agent config         | Draft   | 5      |
| US-003   | Idempotent upgrade and reinstall of an existing kanban install                | Draft   | 5      |
| US-004   | Clean uninstall of the kanban binary, completions, skills, and rc edits       | Draft   | 3      |
| US-005   | Remote `curl \| sh` installer pinned to a git ref                             | Draft   | 5      |
| US-006   | Versioned, checksum-verified release artifacts consumed by the installer     | Draft   | 5      |

---

## Definition of Done (Epic Level)

- [ ] All child User Stories are complete and accepted
- [ ] End-to-end acceptance criteria verified on a clean macOS account and a
      clean Linux account (Debian and Alpine)
- [ ] Architecture Decision Records updated if new decisions were made (release
      artifact naming, signing approach, agent skill install path)
- [ ] README and `HOWTO.md` document the local and remote install commands and
      the standard install locations
- [ ] The workspace version in `Cargo.toml` is bumped per the SemVer scheme in
      `AGENTS.md` for every merged child story
- [ ] `cargo fmt --all --check`, `cargo test`, `cargo clippy --workspace
      --all-targets -- -D warnings`, and `cargo build` all pass
- [ ] `kanban validate .` and `kanban doctor .` pass on this repository
- [ ] Product Owner sign-off received

---

## Notes and Open Questions

| #   | Question / Assumption                                                                  | Owner          | Due        | Resolved |
| --- | -------------------------------------------------------------------------------------- | -------------- | ---------- | -------- |
| 1   | Should release artifacts be built on CI by a tagged release workflow or manually?     | Tooling lead   | 2026-07-04 | No       |
| 2   | Is `~/.local/bin` the right default prefix on macOS, or should it be `~/bin`?          | Tooling lead   | 2026-06-27 | No       |
| 3   | Should the agent skill install path default to `~/.config/opencode/skills/` when no opencode config exists yet? | Tooling lead | 2026-06-27 | No       |
| 4   | Do we ship a single musl-static binary per target, or glibc-dynamic per (arch, libc)? | Tooling lead   | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic Epic template derived from the kanban tooling conventions_
