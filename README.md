# AutoPASS Kanban

An opinionated markdown-first, git-centric, spec-driven, product development management tool. Using epics, user stories and tasks for planning and tracking delivery work. It's particulary useful in large scale projects where a lot of planning is required, to make utilizing AI assistants more effective.

It provides a git-backed kanban board without vendor lock-in, with both CLI and web interfaces.

Optimized for effective use of AI assistants in the workflow, with human-friendly markdown files as the source of truth and machine-readable JSON output for scripting and integration.

## Install from GitHub

Install the latest GitHub release with:

```sh
curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash
```

## What it is

Kanban treats a project's backlog as plain markdown files living in the git
repository alongside the code. Stories, tasks, sprints, epics, and phases are
all human-readable markdown documents with structured YAML frontmatter — there
is no database, hidden state store, or external service. The git history *is*
the audit trail, and the files remain fully editable by hand.

On top of those files the tool provides two interfaces:

- a `kanban` command-line interface (with a documented `kb` alias) for
  inspecting and updating the backlog directly from the terminal, and
- an embedded web UI (`kanban web start`) that serves a Vite/React board from
  the same compiled binary, reading and writing the very same markdown files.

## Purpose

The goal is to keep the backlog as close to the work as possible: versioned,
reviewable, diff-able, and free of vendor lock-in. Because the source of truth
is markdown in git, planning data travels with the repository, survives tooling
changes, and can be branched, reviewed in pull requests, and merged like any
other code. Teams get a kanban workflow without surrendering their data to a
hosted SaaS board.

## What it does

- Models a backlog as markdown stories and tasks, optionally organized into
  sprints, epics, and phases (each concept can be enabled or disabled per repo).
- Lets you list, show, move, plan, and update stories and tasks from the CLI,
  and manage sprints (create, rollover, sync, show current).
- `validate` and `doctor` check the backlog for structural and semantic issues
  without rewriting human-authored content.
- Serves an interactive web board from the same binary for visual planning.
- Emits machine-readable `--format json` envelopes for scripting and CI, with
  exit codes that reflect command status.
- Ships shell completions for bash, zsh, and PowerShell, plus a no-sudo local
  installer and a Docker image for running the web UI without a host install.

CLI binary name:

- `kanban`
- short alias in help/docs: `kb`

## Repository quick start

Run `kanban init` once from the repository root. For this repository, shared
config is already stored in `.kanban/`, so setup is typically only needed for a
fresh clone or if the local config was removed. After that, enable shell
completion and run commands such as `kanban sprint current`, `kanban sprint
sync`, and `kanban story list --current` from the repository root.

## Install from GitHub

Install the latest GitHub release with:

```sh
curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash
```

Pin a specific release with the standard `bash -s --` flag escape hatch:

```sh
curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash -s -- --version v26.6.2201
```

The remote installer detects the current OS and CPU and downloads the matching
`kanban-<version>-<target>.tar.gz` release asset from GitHub Releases. Supported
release targets are:

| OS | CPU | Target triple |
|---|---|---|
| macOS | Intel / AMD64 | `x86_64-apple-darwin` |
| macOS | Apple Silicon / ARM64 | `aarch64-apple-darwin` |
| Linux glibc | Intel / AMD64 | `x86_64-unknown-linux-gnu` |
| Linux musl / Alpine | Intel / AMD64 | `x86_64-unknown-linux-musl` |
| Linux glibc | ARM64 | `aarch64-unknown-linux-gnu` |
| Windows | Intel / AMD64 | `x86_64-pc-windows-msvc` |
| Windows | ARM64 | `aarch64-pc-windows-msvc` |

Every download is verified against `kanban-<version>-checksums.txt` before the
archive is extracted. To verify manually after downloading release assets:

```sh
sh scripts/release/checksums.sh kanban-26.6.2201-*.tar.gz > expected-checksums.txt
sha256sum -c kanban-26.6.2201-checksums.txt
```

## Local install

Install the prebuilt `kanban` binary with a single script:

```sh
sh scripts/install.sh --binary ./target/release/kanban
```

The installer copies the binary to `~/.local/bin/kanban` (default), writes an
install manifest at `~/.local/lib/kanban/manifest.txt`, and asks before making
optional changes such as installing agent skills, editing your shell profile, or
installing tab completions. No `sudo` is required.

### Flags

| Flag | Description |
|---|---|
| `--binary <path>` | Path to the prebuilt `kanban` binary for local install mode |
| `--prefix <dir>` | Install directory for the binary (default: `~/.local/bin`) |
| `--version <tag>` | Install a specific GitHub release, for example `v26.6.2201` |
| `--cache-dir <dir>` | Override the remote artifact cache directory (default: `~/.cache/kanban`) |
| `--offline` | Install from cached remote artifacts only |
| `--skills-dir <dir>` | Install kanban agent skills to a specific directory without prompting |
| `--no-skills` | Skip agent skill installation |
| `--add-path` | Add the install directory to the detected shell profile without prompting |
| `--no-add-path` | Do not edit the shell profile `PATH` |
| `--completions` | Install shell completions for the detected shell without prompting |
| `--no-completions` | Skip shell completion installation |
| `--yes` | Accept default optional integrations without prompting |
| `--dry-run` | Preview all actions without modifying the filesystem |
| `--quiet` | Suppress non-error log lines |

### Examples

```sh
# Install from a release build
sh scripts/install.sh --binary ./target/release/kanban

# Install to a custom directory
sh scripts/install.sh --binary ./kanban --prefix ~/bin

# Install binary, add PATH, completions, and discovered skills without prompts
sh scripts/install.sh --binary ./target/release/kanban --yes

# Preview what would happen without making changes
sh scripts/install.sh --binary ./target/release/kanban --dry-run

# Preview the latest GitHub release install without changing files
curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash -s -- --dry-run

# Install silently
sh scripts/install.sh --binary ./target/release/kanban --quiet
```

After installation, the tool can manage itself:

```sh
# Upgrade to the latest GitHub release using the remote installer
kanban upgrade

# Preview an upgrade without changing files
kanban upgrade --dry-run

# Remove files installed by the kanban installer
kanban uninstall --yes
```

### Supported shells

| Shell | RC file | Completions |
|---|---|---|
| bash | `~/.bashrc` | `~/.local/share/bash-completion/completions/kanban` |
| zsh | `~/.zshrc` | `~/.zsh/completions/_kanban` |
| ash (Alpine) | `~/.profile` | Skipped (no completion support) |
| fish and others | None | Skipped with a notice |

## Release build

Build the optimized release binary with:

```sh
cargo build -p kanban-cli --release
```

The `kanban` binary includes the local web UI server. For a standalone release
that embeds the full Vite web app, build the CLI normally:

```sh
cargo build -p kanban-cli --release
```

Cargo builds the Vite client automatically when `web/dist` is missing or older
than the web sources. If `web/node_modules` is missing, the build script runs
`npm install` first. Runtime production use of `kanban web start` does not
require Node.js or npm; it starts the embedded Rust server from the compiled
`kanban` executable.

For frontend development, run the API server and Vite separately:

```sh
cargo run -p kanban-cli -- web serve --repo-root ../ip-2.0 --host 127.0.0.1 --port 3000
npm --prefix web run dev
```

The Vite app lives in `web/` and proxies `/api` to `http://127.0.0.1:3000` by
default. Override the proxy target with `KANBAN_WEB_API_ORIGIN` when needed.

## Docker

Build and run the standalone CLI plus embedded web UI in a container with:

```sh
./docker-compose.up.sh
```

By default the compose file bind-mounts sibling checkout `../ip-2.0` as `/repo`.
Set `KANBAN_REPO_PATH=/path/to/project` to serve a different git-backed kanban
repository without installing `kanban` on the host.

Implemented commands:

- `kanban init [--no-sprints|--no-epics|--no-phases] [repo_root]`
- `kanban config show [repo_root]`
- `kanban config get <key> [repo_root]`
- `kanban config set <key> <value> [repo_root]`
- `kanban features list [repo_root]`
- `kanban features enable <sprints|epics|phases> [repo_root]`
- `kanban features disable <sprints|epics|phases> [repo_root]`
- `kanban sprint current [repo_root]`
- `kanban sprint list [repo_root]`
- `kanban sprint show <name> [repo_root]`
- `kanban sprint create [--number <n>] [--headline <slug>] [--start <yyyy-mm-dd>] [--end <yyyy-mm-dd>] [--non-interactive] [repo_root]`
- `kanban sprint rollover <name> [repo_root]`
- `kanban sprint sync [repo_root]`
- `kanban phase show <phase> [repo_root]`
- `kanban story show <id> [repo_root]`
- `kanban story list [--current|--all|--next|--sprint <id>] [repo_root]`
- `kanban story move <id> <status> [-a|--assignee "Name <email>"] [repo_root]`
- `kanban story plan <id> --sprint <sprint> [repo_root]`
- `kanban task add <story_id> --title <title> --description <text> [--status <status>] [--tags <a,b>] [repo_root]`
- `kanban task update <story_id> <task_id> [--title <title>] [--description <text>] [--status <status>] [--tags <a,b>] [repo_root]`
- `kanban completion bash`
- `kanban completion zsh`
- `kanban completion powershell`
- `kanban completion help`
- `kanban validate [repo_root]`
- `kanban doctor [repo_root]`
- `kanban doctor help`

## Repository configuration

Run `kanban init` once per repository. This creates `.kanban/settings.json` in the git root with:

- backlog and sprint file locations, defaulting to `delivery/backlog` and `delivery/sprints`
- terminal color behavior
- allowed story point values and alias conversion

If `.kanban/` is missing, operational commands fail with a prompt to run `kanban init`.

### Optional features

The phases, sprints, and epics concepts are all optional. Each can be disabled
when the repository does not organize work that way. Disable features at init
time with `kanban init --no-sprints --no-epics --no-phases`, or toggle them
later with `kanban features disable <sprints|epics|phases>`. The current state
is recorded in `.kanban/settings.json` under the `paths.features` block.

When a feature is disabled:

- The corresponding subcommands (`kanban sprint *`, `kanban epic *`,
  `kanban phase *`) return a clear `feature disabled` error.
- Story frontmatter fields specific to the feature are no longer required
  (`sprint` when sprints are off, `epic` when epics are off).
- `validate` and `doctor` skip the rules that only apply to that feature.

Existing repositories without a `features` block default to all features
on, so the change is fully backward compatible.

## Shell completion

### zsh

Add to `~/.zshrc`:

```zsh
eval "$(kanban completion zsh)"
```

### bash

Add to `~/.bashrc` or `~/.bash_profile`:

```bash
eval "$(kanban completion bash)"
```

### PowerShell

Add to `$PROFILE`:

```powershell
kanban completion powershell | Out-String | Invoke-Expression
```

### Note on direnv

`.envrc` is evaluated as bash by direnv, so `eval "$(kanban completion zsh)"` cannot be
placed there — the zsh-specific completion syntax will fail. The `eval` approach in
`~/.zshrc` is the recommended setup; it runs once per shell regardless of directory.

- `crates/core`: shared parsing and validation core
- `crates/cli`: CLI interface for inspection and lightweight write flows
- `crates/web-server`: embedded Rust web server used by `kanban web start`
- `web`: Vite/React web app source used for development and release-time embedded assets

Run tests with `cargo test` from this directory.

## Terminal output

Human-readable commands use semantic ANSI color when stdout is an interactive
terminal. Color is disabled automatically for pipes, `NO_COLOR`, and `TERM=dumb`
so command output remains safe for scripts and review notes.

## JSON output

Pass `--format json` (before the subcommand) to switch any command to
machine-readable mode. The flag is supported on all read commands (`story show`,
`story list`, `sprint current`, `sprint list`, `sprint show`, `phase show`,
`config show`, `config get`), on the write commands (`story move`, `story plan`,
`task add`, `task update`, `sprint create`, `sprint rollover`, `sprint sync`),
and on `validate` and `doctor`.

Every invocation in JSON mode emits a single envelope on stdout:

```json
{ "status": "ok", "kind": "story.show", "schema_version": 1, "data": { }, "error": null }
```

Fields:

| Field | Values | Meaning |
|---|---|---|
| `status` | `ok` \| `warning` \| `error` | Outcome of the command |
| `kind` | string, e.g. `story.list` | Identifies the response shape |
| `schema_version` | `1` | Envelope version — increment on breaking changes |
| `data` | object or array | Command-specific payload (null on error) |
| `error` | string or null | Error message when `status` is `error` |

Exit codes match status: `ok` → 0, `warning` → 2, `error` → 1. `warning` is
used when `validate` or `doctor` finish successfully but find issues. Human
output remains the default when `--format` is omitted.

Full per-command schema documentation:
`docs/superpowers/specs/2026-06-03-kanban-json-output-design.md` in the served project repository.
