# SPEC: Self-contained local install and `curl | sh` remote install

> Spec/plan for Epic **EP-001: Self-contained local install of the kanban tool
> and its agent skills**. Lives next to the epic so the backlog is the source
> of truth for both the *what* (epic + user stories) and the *how* (this
> document). Update this file alongside story changes; never let it drift from
> the stories.

---

## 1. Goals and non-goals

### Goals

1. A new user can install `kanban` and its agent skills on macOS or Linux in
   under a minute, with no Rust toolchain and no `sudo`, by running one
   command fetched from the git repository.
2. Existing users can re-run the same installer to upgrade or repair their
   install, and can uninstall cleanly when they stop using the tool.
3. AI assistants that discover user-level skills (opencode and similar) pick
   up the two kanban skills without per-project copying.
4. The installer is verifiable: every release artifact has a SHA-256 in a
   checksums file the installer checks before writing to the user's home
   directory.

### Non-goals

- Windows native installer (PowerShell, `winget`, `scoop`). Future Epic.
- System-wide `/usr/local/bin` installs requiring `sudo`. Future Epic
  (Homebrew formula, AUR, Debian package).
- Background auto-update daemon. Upgrades are explicit re-runs.
- PGP / Sigstore signing, transparency log, CDN mirror. Follow-up Epic; this
  Epic ships SHA-256 against a checksums file only.
- Graphical installer or TUI. The installer is a script with flags and a
  small number of yes/no prompts.

---

## 2. Target users and platforms

| User                              | Why they care                                              |
| --------------------------------- | ---------------------------------------------------------- |
| New user on a clean machine       | Wants `kanban` on PATH fast, no clone, no toolchain         |
| Existing contributor              | Wants to upgrade or repair without manual cleanup           |
| AI assistant user (opencode etc)  | Wants the kanban skills available in every repository       |
| Tooling lead / release pipeline   | Wants reproducible, auditable release artifacts             |

| Platform                             | Shell scope        | Libc            | Status     |
| ------------------------------------ | ------------------ | --------------- | ---------- |
| macOS, Apple Silicon                 | bash, zsh          | system          | Supported  |
| macOS, Intel                         | bash, zsh          | system          | Supported  |
| Linux x86_64, Debian/Ubuntu/Fedora   | bash               | glibc           | Supported  |
| Linux x86_64, Alpine                 | ash, bash          | musl            | Supported  |
| Linux aarch64, glibc distros         | bash               | glibc           | Supported  |
| Linux aarch64, Alpine                | ash, bash          | musl            | Best-effort; ship if the cross-compile works in CI |
| Windows                              | PowerShell, cmd    | —               | Out of scope |
| Other (BSD, Illumos, armv7, etc.)    | —                  | —               | Out of scope; installer errors clearly |

---

## 3. Standard install locations

The installer uses XDG-ish user-local locations. Every path is overridable;
the values below are defaults.

| Artifact                       | Default path                                                | Override flag            |
| ------------------------------ | ----------------------------------------------------------- | ------------------------ |
| `kanban` binary                | `~/.local/bin/kanban`                                       | `--prefix <dir>`         |
| Install manifest               | `~/.local/lib/kanban/manifest.txt`                          | Derived from `--prefix`  |
| Bash completion                | `~/.local/share/bash-completion/completions/kanban`         | `--bash-completion <dir>`|
| Zsh completion                 | `~/.zsh/completions/_kanban`                                | `--zsh-completion <dir>` |
| Agent skills                   | Discovered per §5                                           | `--skills-dir <dir>`     |
| Download cache (US-005 only)   | `~/.cache/kanban/`                                          | `--cache-dir <dir>`      |

### 3.1 macOS prefix choice

Open Question 2 in the epic asks whether `~/.local/bin` or `~/bin` is the
right default prefix on macOS. Proposal: **`~/.local/bin`** on both macOS and
Linux, for consistency. macOS users who already have `~/bin` on `PATH` can
pass `--prefix ~/bin`. The installer logs which prefix it chose and why.

### 3.2 XDG environment variables honored

| Variable             | Effect on installer                                                   |
| -------------------- | --------------------------------------------------------------------- |
| `XDG_CONFIG_HOME`    | Replaces `~/.config` in skill discovery (§5) and manifest path        |
| `XDG_DATA_HOME`      | Replaces `~/.local/share` for bash completion default                 |
| `XDG_CACHE_HOME`     | Replaces `~/.cache` for the download cache (US-005)                   |
| `XDG_BIN_HOME`       | Replaces `~/.local/bin` for the binary default if set                 |
| `HOME`               | Resolves `~` for all paths                                            |

---

## 4. `scripts/install.sh` design

### 4.1 File layout

```
scripts/
  install.sh                 # the installer (POSIX sh)
  uninstall.sh               # the uninstaller (POSIX sh)
  release/
    checksums.sh             # builds kanban-<version>-checksums.txt from tarballs
tests/
  install/
    fixtures/                # stub `kanban` binary, fixture HOME tree
    install.bats             # end-to-end install tests
    upgrade.bats             # re-run / upgrade tests
    uninstall.bats           # uninstall tests
    remote.bats              # curl | sh tests against a local HTTP fixture
```

If `install.sh` grows past ~400 lines, split helpers into
`scripts/install/` and `source` them; do not split prematurely.

### 4.2 Top-level structure

```sh
#!/bin/sh
set -eu
umask 022

INSTALLER_VERSION="<workspace version from Cargo.toml at build time>"

# 1. Parse flags
# 2. Detect platform (uname -s, uname -m, libc)
# 3. Resolve binary source:
#    a. --binary <path>          -> local file
#    b. --version <tag> / --channel main -> remote download (US-005)
#    c. neither                  -> usage error
# 4. Discover skill dir (US-002) unless --no-skills
# 5. Reconcile against previous manifest (US-003) if any
# 6. Stage and atomically install binary, completions, skills
# 7. Apply sentinel-guarded rc edits
# 8. Rewrite manifest
# 9. Print summary
```

### 4.3 Flags

| Flag                              | Purpose                                                      | Default        |
| --------------------------------- | ------------------------------------------------------------ | -------------- |
| `--binary <path>`                 | Install from a local prebuilt binary (US-001)                | unset          |
| `--version <tag>`                 | Remote install pinned to a release tag (US-005)              | unset          |
| `--channel main`                  | Remote install from latest main CI artifact (US-005)         | unset          |
| `--prefix <dir>`                  | Binary install prefix                                        | `~/.local/bin` |
| `--skills-dir <dir>`              | Skill install dir; skips discovery prompt (US-002)           | discovered     |
| `--no-skills`                     | Skip skill install entirely                                  | off            |
| `--no-completions`                | Skip completion install                                      | off            |
| `--bash-completion <dir>`         | Override bash completion dir                                 | derived        |
| `--zsh-completion <dir>`          | Override zsh completion dir                                  | derived        |
| `--cache-dir <dir>`               | Override download cache (US-005)                             | `~/.cache/kanban` |
| `--offline`                       | Use cache only, no network (US-005)                          | off            |
| `--yes`                           | Accept all defaults without prompting; CI-friendly           | off            |
| `--force`                         | Allow downgrade and overwrite of user-edited files           | off            |
| `--dry-run`                       | Preview every filesystem write and rc edit; do nothing       | off            |
| `--quiet`                         | Suppress non-error log lines                                 | off            |
| `--verbose`                       | Per-step detail                                              | off            |
| `--help`                          | Usage                                                        | —              |

### 4.4 Install manifest format

Plain text, one record per line, tab-separated. Header lines start with `#`.

```
# kanban install manifest
# installer-version: 26.7.0101
# installed-at: 2026-06-21T16:48:56+0200
# prefix: /Users/tm/.local
# skills-dir: /Users/tm/.config/opencode/skills
~/.local/bin/kanban	<sha256>	local:./target/release/kanban	26.7.0101
~/.local/share/bash-completion/completions/kanban	<sha256>	generated:kanban-completion-bash	26.7.0101
~/.zsh/completions/_kanban	<sha256>	generated:kanban-completion-zsh	26.7.0101
~/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md	<sha256>	repo:skills/kanban-backlog-maintainer/SKILL.md	26.7.0101
~/.config/opencode/skills/kanban-backlog-maintainer/plugin.json	<sha256>	repo:skills/kanban-backlog-maintainer/plugin.json	26.7.0101
~/.config/opencode/skills/kanban-developer/SKILL.md	<sha256>	repo:skills/kanban-developer/SKILL.md	26.7.0101
~/.config/opencode/skills/kanban-developer/plugin.json	<sha256>	repo:skills/kanban-developer/plugin.json	26.7.0101
```

For remote installs the `source` field is `remote:<artifact-url>;sha256=<hex>`.

### 4.5 Sentinel-commented shell rc edits

Every rc line the installer adds is tagged with a `# kanban-installer:` sentinel
so the uninstaller can remove exactly those lines.

Bash example:

```sh
export PATH="$HOME/.local/bin:$PATH"  # kanban-installer: path
```

Zsh example (completions):

```sh
fpath=(~/.zsh/completions $fpath)     # kanban-installer: fpath
autoload -Uz compinit && compinit     # kanban-installer: compinit
```

The installer:

1. Reads the rc file if it exists.
2. Skips the edit if a line with the matching sentinel is already present.
3. Appends the new line with a trailing sentinel comment.
4. Writes via a temp file and atomic `mv` so a crash cannot corrupt the rc file.

### 4.6 Atomic file replacement

Binary and manifest are installed atomically:

```sh
tmp="<target>.new.$$"
cp "$src" "$tmp"
chmod +x "$tmp"
mv -f "$tmp" "$target"
```

Temp files are created in the same directory as the target so `mv` stays on
the same filesystem. On cross-filesystem `--prefix` (unusual), the installer
falls back to `cp` + `fsync` + `mv` and warns.

### 4.7 Reconciliation (US-003)

Before writing anything, the installer:

1. Reads the previous manifest if present.
2. Computes the planned file list for this run.
3. Diffs previous vs planned into three lists:
   - `to_overwrite` — in both lists; will be replaced.
   - `to_add` — in planned only; will be created.
   - `to_remove` — in previous only; will be `rm`'d.
4. For each `to_overwrite`, checks the on-disk hash against the previous
   manifest hash. On mismatch (user edited the file), prompt before
   overwriting unless `--force`.
5. Writes the new state, then rewrites the manifest.

### 4.8 Downgrade detection (US-003)

The installer compares `kanban --version` from the existing binary against the
candidate binary before replacing it. If the candidate is older, it prompts
`Downgrade from <old> to <new>? [y/N]` unless `--force`.

---

## 5. Agent skill discovery and install (US-002)

### 5.1 Discovery priority

The installer resolves the skill install directory in this order:

1. `--skills-dir <dir>` flag (skip discovery and prompt).
2. `$OPENCODE_HOME/skills/` if `OPENCODE_HOME` is set.
3. `$XDG_CONFIG_HOME/opencode/skills/` if `XDG_CONFIG_HOME` is set.
4. `~/.config/opencode/skills/` if it already exists.
5. `~/.local/share/opencode/skills/` if it already exists.
6. Prompt the user with `~/.config/opencode/skills/` as the proposed default.

If discovery finds no existing config and the user does not provide a path,
the proposed default is `~/.config/opencode/skills/`. The user can accept,
type an alternative path, or skip with `--no-skills`.

### 5.2 Confirmation prompt

```text
kanban installer: no existing agent config found.

Checked:
  - $OPENCODE_HOME/skills/        (unset)
  - $XDG_CONFIG_HOME/opencode/skills/  (unset)
  - ~/.config/opencode/skills/    (does not exist)
  - ~/.local/share/opencode/skills/    (does not exist)

Install kanban skills to ~/.config/opencode/skills/? [Y/n/path]
```

- Reads from `/dev/tty` so `curl | sh` can still prompt.
- In non-interactive mode (`[ -t 0 ]` is false or `--yes` is set), accepts the
  proposed default without prompting and prints a single-line notice.

### 5.3 Files installed

For each skill (`kanban-backlog-maintainer`, `kanban-developer`):

```
<skills-dir>/<skill-name>/SKILL.md
<skills-dir>/<skill-name>/plugin.json
```

The installer copies from the canonical `skills/<skill-name>/` directory in
the repository (or the `skills/` directory inside the release tarball for
remote installs). It does **not** copy the `.agents/skills/` symlinks — those
are repository-local scaffolding.

### 5.4 Upgrade-in-place

If the target file already exists:

1. Compare on-disk hash to previous manifest hash.
2. If they match, overwrite silently (the user has not edited it).
3. If they differ, prompt `Local edits to <path> will be overwritten. Continue? [y/N]`
   unless `--force`.
4. Print a one-line diff summary (`+N -M lines`).

---

## 6. Uninstaller (`scripts/uninstall.sh`)

### 6.1 Flow

```sh
1. Parse flags (--prefix, --skills-dir, --yes, --dry-run, --force)
2. Read <prefix>/lib/kanban/manifest.txt; abort if missing (Scenario 4)
3. For each file in the manifest:
   a. Compute on-disk hash.
   b. If it matches the manifest hash, rm -f.
   c. If it differs, skip with a warning (Scenario 3).
4. For each rc file touched by the installer:
   a. Strip lines tagged "# kanban-installer:".
   b. Write via temp file + atomic mv.
5. Prompt for skill removal (Scenario 6) unless --yes.
6. rm -rf <prefix>/lib/kanban/ (manifest directory).
7. Print summary: removed, skipped, rc files edited, orphans noticed.
```

### 6.2 Safety

- Never follow symlinks when removing (`rm -f` on the path itself).
- Refuse to remove paths that escape the prefix or skills dir after
  expansion (no `..` traversal).
- Skip and warn on hash mismatch rather than removing user-edited files.
- Atomic rc edits so a crash cannot corrupt `.bashrc` / `.zshrc`.

---

## 7. Remote install (`curl | sh`, US-005)

### 7.1 Public commands

Pinned to a release tag (recommended):

```sh
curl -fsSL https://<repo>/install.sh | sh -s -- --version v26.7.0101
```

Latest release tag:

```sh
curl -fsSL https://<repo>/install.sh | sh
```

From `main` (not reproducible, warns):

```sh
curl -fsSL https://<repo>/install.sh | sh -s -- --channel main --yes
```

With overrides:

```sh
curl -fsSL https://<repo>/install.sh | sh -s -- --version v26.7.0101 --prefix ~/bin --dry-run
```

The README documents the `sh -s -- <flags>` pattern because flags cannot be
passed to `curl` directly (curl's stdout is the script).

### 7.2 Target detection

```sh
detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin) case "$arch" in
              arm64)  echo "aarch64-apple-darwin" ;;
              x86_64) echo "x86_64-apple-darwin"  ;;
              *)      return 1 ;;
            esac ;;
    Linux)  case "$arch" in
              x86_64) libc_flavour; case "$libc" in
                        musl) echo "x86_64-unknown-linux-musl" ;;
                        gnu)  echo "x86_64-unknown-linux-gnu"  ;;
                      esac ;;
              aarch64) libc_flavour; case "$libc" in
                        musl) echo "aarch64-unknown-linux-musl" ;;
                        gnu)  echo "aarch64-unknown-linux-gnu"  ;;
                      esac ;;
              *)      return 1 ;;
            esac ;;
    *)      return 1 ;;
  esac
}
```

`libc_flavour` checks for `/lib/ld-musl-*.so.1` (musl) vs `/lib/ld-linux*.so*`
(glibc). On ambiguous systems the user can override with
`--target <triple>` (undocumented escape hatch; not required for the happy
path).

### 7.3 Version resolution

| Input                          | Resolved URL                                                                  |
| ------------------------------ | ------------------------------------------------------------------------------ |
| `--version v26.7.0101`         | `<repo>/releases/download/v26.7.0101/kanban-26.7.0101-<triple>.tar.gz`         |
| `--version 26.7.0101`          | Same as above (leading `v` optional)                                           |
| (no `--version`, no `--channel`) | Query the latest release tag via the git host's releases API; use that.     |
| `--channel main`               | `<repo>/releases/download/nightly/kanban-<commit>-<triple>.tar.gz` (CI-published nightly) |

The checksums file for a release is always at
`<repo>/releases/download/v<version>/kanban-<version>-checksums.txt`.

### 7.4 Download, verify, extract flow

```sh
fetch_and_verify() {
  version="$1"; target="$2"
  cache="${KANBAN_CACHE_DIR:-$HOME/.cache/kanban}"
  mkdir -p "$cache"
  tarball="kanban-$version-$target.tar.gz"
  checksums="kanban-$version-checksums.txt"
  base="<repo>/releases/download/v$version"
  # 1. Download (or use cache if --offline)
  # 2. Compute sha256 of the tarball
  # 3. Look up the tarball name in the checksums file; abort if missing
  # 4. Compare hashes; abort if mismatch
  # 5. Extract to a temp dir
  # 6. Print the extracted dir path
}
```

Cache layout:

```
~/.cache/kanban/
  kanban-26.7.0101-x86_64-apple-darwin.tar.gz
  kanban-26.7.0101-checksums.txt
  kanban-26.7.0101-x86_64-apple-darwin.tar.gz.sha256   # computed on first download
```

`--offline` skips the network and uses whatever is in the cache; the checksum
verification still runs against the cached checksums file.

### 7.5 Non-interactive defaults (CI)

When `stdin` is not a TTY:

- `--yes` is implied for the skill discovery prompt; the discovered default is
  used (or `~/.config/opencode/skills/` if nothing was discovered).
- Downgrade prompts default to `N` (refuse) unless `--force` is also set.
- Local-edit prompts default to `N` (skip) unless `--force` is set.

The installer prints a single-line summary of each default it took.

### 7.6 Failure modes

| Failure                          | Exit status | Behavior                                                              |
| -------------------------------- | ----------- | --------------------------------------------------------------------- |
| Unsupported target               | 1           | Print "No release artifact for target <triple>"; do not write to HOME |
| Checksum mismatch                | 1           | Print expected vs actual; remove temp download; do not write to HOME  |
| Checksums file missing the entry| 1           | Print "Checksums file does not list <tarball>"; do not write to HOME  |
| Network unreachable, no cache   | 1           | Print "Network unreachable and no cached artifact; try --offline with a previously downloaded tarball" |
| Disk full mid-extract            | 1           | Remove temp dir; do not invoke install flow                           |

---

## 8. Release artifacts (US-006)

### 8.1 Naming contract

| Artifact                                              | Producer                |
| ----------------------------------------------------- | ----------------------- |
| `kanban-<version>-<target-triple>.tar.gz`             | CI release workflow     |
| `kanban-<version>-checksums.txt`                      | CI release workflow     |

`<version>` is the workspace version from `Cargo.toml` (e.g. `26.7.0101`),
matching the git tag without the leading `v`.

### 8.2 Tarball layout

```
kanban-<version>-<triple>.tar.gz
  kanban                                    # executable
  VERSION                                   # one line: <version>
  skills/
    kanban-backlog-maintainer/
      SKILL.md
      plugin.json
    kanban-developer/
      SKILL.md
      plugin.json
```

No other files. The layout is identical across target tarballs except for the
binary itself, so the installer's extraction code is target-independent.

### 8.3 Checksums file format

Standard `sha256sum` format, sorted by filename, two-space separation:

```
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  kanban-26.7.0101-aarch64-apple-darwin.tar.gz
<sha256>  kanban-26.7.0101-aarch64-unknown-linux-gnu.tar.gz
<sha256>  kanban-26.7.0101-aarch64-unknown-linux-musl.tar.gz
<sha256>  kanban-26.7.0101-x86_64-apple-darwin.tar.gz
<sha256>  kanban-26.7.0101-x86_64-unknown-linux-gnu.tar.gz
<sha256>  kanban-26.7.0101-x86_64-unknown-linux-musl.tar.gz
```

This format is verifiable with `sha256sum -c checksums.txt` on Linux and
`shasum -a 256 -c checksums.txt` on macOS.

### 8.4 Release workflow

`.github/workflows/release.yml` (or equivalent):

1. Trigger: tag push matching `v<workspace version>`.
2. Build matrix: one job per target triple.
3. Each job:
   - `rustup target add <triple>`
   - `cargo build -p kanban-cli --release --target <triple>`
   - Pack `kanban-<version>-<triple>.tar.gz` with `kanban`, `VERSION`, and
     `skills/`.
   - Upload the tarball as a release asset.
4. Post-build job (depends on all build jobs):
   - Download every uploaded tarball.
   - Run `scripts/release/checksums.sh <version>` to produce the checksums file.
   - Upload `kanban-<version>-checksums.txt` as a release asset.
5. Verification job (post-release):
   - Dry-run `scripts/install.sh --version v<version> --dry-run` against the
     just-published release.
   - Confirm the installer can resolve, verify, and plan the install.

A failed build for one target does not block the others (Scenario 8 in
US-006). The checksums file lists only the tarballs that were successfully
uploaded.

### 8.5 Reproducibility

- CI pins the Rust toolchain via `rust-toolchain.toml`.
- `scripts/release/checksums.sh` rebuilds the checksums file from local
  tarballs so a maintainer can audit a published checksums file against a
  manual rebuild.
- The release notes record the CI workflow run URL and the commit SHA.

---

## 9. End-to-end request graph (curl | sh)

```text
User
 |
 |  curl -fsSL https://<repo>/install.sh | sh -s -- --version v26.7.0101
 v
Remote git host
 |
 |  scripts/install.sh (raw file at the tag or main)
 v
Local sh (running the installer)
 |
 |  1. detect_target()                          -> "aarch64-apple-darwin"
 |  2. resolve_version(v26.7.0101)              -> release URL
 |  3. fetch_and_verify():
 |       GET <repo>/releases/download/v26.7.0101/kanban-26.7.0101-checksums.txt
 |       GET <repo>/releases/download/v26.7.0101/kanban-26.7.0101-aarch64-apple-darwin.tar.gz
 |       sha256 < tarball; compare to checksums file
 |       extract to ~/.cache/kanban/kanban-26.7.0101-aarch64-apple-darwin/
 |  4. discover_skills_dir()                    -> prompt or --skills-dir
 |  5. reconcile_manifest()                     -> to_overwrite / to_add / to_remove
 |  6. atomic install:
 |       stage ~/.local/bin/.kanban.new.<pid>
 |       mv -> ~/.local/bin/kanban
 |       install completions
 |       install skills
 |  7. apply sentinel-guarded rc edits to .zshrc
 |  8. rewrite ~/.local/lib/kanban/manifest.txt
 |  9. print summary
 v
Done: `kanban --version` works in a new shell
```

---

## 10. Testing strategy

### 10.1 Layers

| Layer        | Scope                                              | Harness                  |
| ------------ | -------------------------------------------------- | ------------------------ |
| Unit         | `detect_target`, `discover_skills_dir`, `reconcile_manifest`, `strip_sentinel_lines`, `remove_if_hash_matches` | `bats` or a small POSIX `sh` test driver |
| Integration  | Full install / upgrade / uninstall in a fixture HOME with a stub `kanban` | `tests/install/*.bats` |
| Remote       | `curl \| sh` against a local HTTP fixture serving a prebuilt tarball + checksums | `tests/install/remote.bats` with `python3 -m http.server` |
| Release      | Dry-run the installer against the just-published release | CI post-release job |

### 10.2 Fixture HOME

Each integration test:

1. Creates a temp `HOME` (`$BATS_TMPDIR/home.<test>`).
2. Copies the stub `kanban` binary that prints a fixed version string.
3. Runs the installer with explicit `HOME` and `--prefix` pointing at the
   fixture.
4. Asserts the resulting tree, rc file contents, and manifest contents.

### 10.3 CI matrix

Run integration tests on:

- macOS (latest, Apple Silicon and Intel)
- Ubuntu (latest, x86_64, glibc)
- Alpine (latest, x86_64, musl) — via container

### 10.4 Failure-path tests

Every failure scenario in the stories is covered:

- Checksum mismatch aborts before any filesystem write.
- Unsupported target errors clearly.
- Non-interactive defaults are taken when stdin is not a TTY.
- `SIGINT` mid-install leaves the previous install intact.
- Re-run with no changes writes nothing.
- Uninstall skips user-edited files.

---

## 11. Documentation updates

| Document         | Update                                                                          |
| ---------------- | ------------------------------------------------------------------------------- |
| `README.md`      | Add an "Install" section with local and remote commands, flags, and verification |
| `HOWTO.md`       | Add a "Getting `kanban` on your machine" walkthrough with troubleshooting       |
| `AGENTS.md`      | Add a note in the verification section: after installer changes, run `tests/install/*.bats` |
| `SPEC-installability.md` (this file) | Updated alongside story changes; never drifts from the stories |

---

## 12. Implementation sequencing

The stories can land in this order, with US-005 and US-006 landing together:

1. **US-001** — local binary install with PATH and completion bootstrap.
2. **US-002** — agent skill discovery and install, layered on US-001's installer.
3. **US-003** — idempotent upgrade, layered on US-001 + US-002's manifest.
4. **US-004** — clean uninstall, a pure consumer of the manifest and sentinel rc edits.
5. **US-005 + US-006** — remote `curl | sh` flow and the release artifacts it depends on. These two must land together; a remote installer without checksum verification is irresponsible.

Each story bumps the workspace version in `Cargo.toml` per the SemVer scheme
in `AGENTS.md` and passes the full verification block from `AGENTS.md`:

```sh
cargo fmt --all -- --check
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cargo build
cargo run -p kanban-cli -- validate .
cargo run -p kanban-cli -- doctor .
```

---

## 13. Open questions

Carried from the epic and the stories. Resolve before or during implementation.

| #   | Question                                                                          | Owner        | Default proposal if unresolved                |
| --- | --------------------------------------------------------------------------------- | ------------ | --------------------------------------------- |
| 1   | Should release artifacts be built on CI by a tagged release workflow or manually?| Tooling lead | CI workflow on tag push                       |
| 2   | Is `~/.local/bin` the right default prefix on macOS, or should it be `~/bin`?     | Tooling lead | `~/.local/bin` on both macOS and Linux        |
| 3   | Should the agent skill install path default to `~/.config/opencode/skills/`?     | Tooling lead | Yes, when no existing config is discovered    |
| 4   | Single musl-static binary per target, or glibc-dynamic per (arch, libc)?          | Tooling lead | Both: ship `*-linux-gnu` and `*-linux-musl`   |
| 5   | `scripts/install.sh` one file or split into `scripts/install/`?                   | Tooling lead | One file; split only if it exceeds ~400 lines |
| 6   | Cache at `~/.cache/kanban/` (XDG) or `~/.local/cache/kanban/`?                    | Tooling lead | `~/.cache/kanban/` honoring `XDG_CACHE_HOME`  |
| 7   | Do we mirror artifacts to a CDN, or serve from the git host directly?             | Tooling lead | Git host directly; CDN is a follow-up Epic    |
| 8   | Should the installer touch `.profile` on Debian, or only shell-specific rc files? | Tooling lead | Shell-specific rc files; `.profile` only for `ash` |
| 9   | Do we install completions for both bash and zsh, or only the detected shell?      | Tooling lead | Detected shell only; `--no-completions` skips |

---

## 14. Out-of-scope follow-ups (future Epics)

- Windows native installer (PowerShell, `winget`, `scoop`).
- System-wide `/usr/local/bin` install via package managers (Homebrew formula, AUR, Debian package, `cargo install`).
- PGP / Sigstore signing and a transparency log for release artifacts.
- CDN mirror for release artifacts.
- Background auto-update daemon.
- A `kanban install self` subcommand that wraps `scripts/install.sh` so users
  who already have `kanban` can self-upgrade without `curl | sh`.
- Installing skills into project-local `.opencode/agents/` directories.
- SBOM (CycloneDX or SPDX) generation alongside the checksums file.

---

_Spec version: 1.0 (2026-06-21) — companion to EP-001._
