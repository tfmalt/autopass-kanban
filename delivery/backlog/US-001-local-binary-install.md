---
id: US-001
type: user-story
status: done
epic: EP-001
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 8
work_started: 2026-06-21T18:01:31+0200
work_done: 2026-06-21T21:13:37+0200
created: 2026-06-21T16:48:56+0200
updated: 2026-06-21T21:13:37+0200
---

# User Story: Local install of the kanban binary with PATH and completion bootstrap

---

## Story Statement

**As a** developer on macOS or Linux,
**I want** to run a single `scripts/install.sh` script that places the prebuilt
`kanban` binary into a standard user-local `bin` directory, ensures that
directory is on my shell's `PATH`, and installs shell completions for my
detected shell,
**so that** `kanban` works with tab completion in a freshly opened shell with no
manual PATH editing, no `sudo`, and no Rust toolchain.

---

## Background

Today the only way to get a working `kanban` binary is to clone this repository
and run `cargo build -p kanban-cli --release`. That bars anyone who is not
already a Rust contributor. Even contributors who rebuild frequently have no
shared way to put the binary on `PATH` or wire up completions. This story
delivers the local half of the install flow: a POSIX `sh` script that takes a
prebuilt binary (from a local path or, in later stories, a downloaded release
artifact) and installs it into the conventional user-local location, while
bootstrapping `PATH` and completions so the tool is immediately usable.

This story does not cover downloading artifacts from a remote git repository
(US-005), checksum verification (US-006), agent skill installation (US-002), or
uninstall (US-004). It must, however, leave the script structured so those
stories can be layered on without a rewrite.

---

## Acceptance Criteria

**Scenario 1: Fresh install on macOS with default prefix**

```gherkin
Given a clean macOS user account with Homebrew bash and zsh installed
  and a prebuilt `kanban` binary available at a local path passed via --binary
When the user runs `sh scripts/install.sh --binary ./target/release/kanban`
Then the installer copies the binary to `~/.local/bin/kanban`
  and makes it executable
  and `~/.local/bin` is added to `PATH` in the user's shell rc file
    (`.zshrc` for zsh, `.bashrc` for bash) only if it was not already present
  and a new login shell can run `kanban --version` successfully
```

**Scenario 2: Fresh install on Linux (Debian) with default prefix**

```gherkin
Given a clean Debian user account with bash as the login shell
  and a prebuilt `kanban` binary available at a local path
When the user runs `sh scripts/install.sh --binary ./target/release/kanban`
Then the installer copies the binary to `~/.local/bin/kanban`
  and appends a guarded `export PATH="$HOME/.local/bin:$PATH"` line to `~/.bashrc`
    only if `~/.local/bin` is not already on `PATH` in that file
  and a new login shell can run `kanban --version` successfully
```

**Scenario 3: Fresh install on Linux (Alpine) with busybox ash**

```gherkin
Given a clean Alpine user account whose default `$SHELL` is `/bin/ash`
  and a prebuilt `kanban` binary available at a local path
When the user runs `sh scripts/install.sh --binary ./target/release/kanban`
Then the installer copies the binary to `~/.local/bin/kanban`
  and writes the PATH export to `~/.profile` (the file `ash` sources on login)
  and a new login shell can run `kanban --version` successfully
```

**Scenario 4: Custom prefix via flag**

```gherkin
Given the user wants the binary installed at `~/bin` instead of `~/.local/bin`
When the user runs `sh scripts/install.sh --binary ./target/release/kanban --prefix ~/bin`
Then the installer copies the binary to `~/bin/kanban`
  and adds `~/bin` (not `~/.local/bin`) to `PATH` in the detected shell rc file
  and does not touch `~/.local/bin`
```

**Scenario 5: Shell completion installation for bash**

```gherkin
Given bash is the detected shell
  and `kanban completion bash` produces a valid completion script on stdout
When the installer runs
Then it writes the bash completion script to
  `~/.local/share/bash-completion/completions/kanban`
  (or the directory `$BASH_COMPLETION_USER_DIR/completions` if that env var is set)
  and a new bash shell completes `kanban <Tab>` with the available subcommands
```

**Scenario 6: Shell completion installation for zsh**

```gherkin
Given zsh is the detected shell
  and `kanban completion zsh` produces a valid completion script on stdout
When the installer runs
Then it writes the zsh completion script to `~/.zsh/completions/_kanban`
  and adds `~/.zsh/completions` to `$fpath` in `~/.zshrc` only if it is not already on `fpath`
  and ensures `compinit` is called in `~/.zshrc` only if it is not already called
  and a new zsh shell completes `kanban <Tab>` with the available subcommands
```

**Scenario 7: Unsupported shell is skipped, not failed**

```gherkin
Given the user's `$SHELL` is `fish` (or another shell with no completion installer)
When the installer runs
Then it installs the binary and PATH bootstrap as usual
  and prints a single-line notice that completion installation was skipped for `fish`
  and exits with status 0
```

**Scenario 8: Dry run previews every filesystem write and rc edit**

```gherkin
Given the user runs `sh scripts/install.sh --binary ./target/release/kanban --dry-run`
Then the installer prints every file it would write and every rc line it would append
  and does not modify the filesystem
  and exits with status 0
```

**Scenario 9: Missing binary argument errors cleanly**

```gherkin
Given the user runs `sh scripts/install.sh` without `--binary`
  and without any remote-download flag (introduced in US-005)
Then the installer prints a usage message to stderr
  and exits with status 2 without modifying the filesystem
```

---

## Non-Functional Requirements

| Area                | Requirement                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------- |
| **Portability**     | Script starts with `#!/bin/sh` and uses only POSIX features for the core flow; bash-only helpers guarded by `if [ -n "${BASH_VERSION:-}" ]` |
| **Security**        | Never invokes `sudo`; refuses to write outside the discovered prefix; creates directories with `umask 022` |
| **Traceability**    | Logs every file write and every rc edit to stderr with a `+ ` prefix; `--quiet` suppresses non-error log lines |
| **Auditability**    | After install, writes `<prefix>/lib/kanban/manifest.txt` (default `~/.local/lib/kanban/manifest.txt`) listing each installed file path, its source hash, and the installer version |
| **Performance**     | A fresh install from a local binary completes in under 2 seconds on a warm machine |
| **Backward compatibility** | If `manifest.txt` already exists, the installer reads it before overwriting so US-003 can upgrade cleanly |

---

## Technical Notes

- **Requirement refs:** `EP-001#acceptance-criteria`
- **Component / Module:** `scripts/install.sh` (new); the install manifest lives at `<prefix>/lib/kanban/manifest.txt` and is owned by the installer only — `kanban` itself does not read it at runtime.
- **Key integration points:** consumes `kanban completion bash` and `kanban completion zsh` stdout; consumes the prebuilt binary at the path given by `--binary`.
- **Suggested patterns:**
  - Single-file POSIX `sh` script with `set -eu` and explicit `umask 022`.
  - All destructive operations factored into `do_copy`, `do_append_rc`, and `do_write_completion` helpers that check a `$DRY_RUN` flag, so `--dry-run` is a single branch in each helper.
  - Shell detection by inspecting `$SHELL` and the parent process name; fall back to "no completion" rather than guessing.
  - Rc edits use a sentinel comment (`# kanban-installer: <purpose>`) so US-004 can revert exactly the lines the installer added.
- **Data model hints:** The manifest file format is plain text, one record per line, tab-separated: `<installed-path>\t<sha256>\t<source>\t<installer-version>`. Skills and completions installed by later stories append to the same manifest.
- **Testing approach:**
  - Unit-test the pure helper functions (path discovery, rc edit idempotency, manifest format) with a small `sh` test harness or `bats`.
  - Integration-test the full script in a clean HOME fixture directory under `tests/install/` using a stub `kanban` binary that prints a fixed version string.
  - Run the integration test on macOS, Debian, and Alpine in CI.
- **Migration / backward compatibility:** No prior install path exists; the manifest format should be forward-compatible with US-003 upgrades.

### Estimation Rules

`story_points` is `8` (L): the script itself is small but the cross-shell rc handling, completion directory discovery, and macOS/Linux/Alpine portability testing add real surface area.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` will be set when this story moves to `in-progress`.
- `assignee` reflects the tooling lead who owns the installer.

---

## Definition of Done

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] `scripts/install.sh` exists, is executable, and starts with `#!/bin/sh`
- [ ] Code reviewed and approved via pull request (minimum 1 reviewer)
- [ ] Unit tests for helper functions pass on macOS and Linux CI
- [ ] Integration test in `tests/install/` passes on macOS, Debian, and Alpine CI runners
- [ ] No new static analysis issues introduced (or justified exceptions documented)
- [ ] README and `HOWTO.md` document `sh scripts/install.sh --binary <path>` and the `--prefix`, `--dry-run`, and `--quiet` flags
- [ ] Workspace version bumped in `Cargo.toml` per the SemVer scheme in `AGENTS.md`
- [ ] `cargo fmt --all --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Dependencies

| Dependency                                  | Type           | Status    | Notes                                                              |
| ------------------------------------------- | -------------- | --------- | ------------------------------------------------------------------ |
| Prebuilt `kanban` binary for the target OS  | Infrastructure | Available | For local testing `cargo build -p kanban-cli --release` produces it |
| `kanban completion bash` / `zsh` subcommand | Tool           | Available | Already shipped                                                    |
| US-006: Checksum-verified release artifacts | Story          | Draft     | Only required for the remote download flow; US-001 works from a local `--binary` path independently |

---

## Sprint Task Log Guidance

Expected tasks once activated into a sprint:

- Scaffold `scripts/install.sh` with flag parsing and `--dry-run` helpers
- Implement binary copy and `chmod +x`
- Implement shell rc detection and sentinel-guarded PATH append
- Implement bash completion install with `$BASH_COMPLETION_USER_DIR` handling
- Implement zsh completion install with `$fpath` and `compinit` handling
- Implement `manifest.txt` write
- Add `tests/install/` fixture with a stub `kanban` binary
- Document the command in README and HOWTO.md

---

## Notes and Open Questions

| #   | Question / Assumption                                                                   | Owner        | Due        | Resolved |
| --- | --------------------------------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Is `~/.local/bin` the right default prefix on macOS, or should it be `~/bin`?           | Tooling lead | 2026-06-27 | Yes — `~/.local/bin` chosen per XDG conventions |
| 2   | Should the installer touch `.profile` on Debian, or only the shell-specific rc file?    | Tooling lead | 2026-06-27 | Yes — shell-specific rc file only (`.bashrc` on Debian) |
| 3   | Do we install completions for the user's current shell only, or for bash and zsh both?  | Tooling lead | 2026-06-27 | Yes — current shell only; re-run for other shells |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
