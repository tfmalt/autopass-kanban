# Kanban

Rust workspace for repository-local, markdown-first kanban tooling.

CLI binary name:
- `kanban`
- short alias in help/docs: `kb`

Implemented commands:
- `kanban init [repo_root]`
- `kanban config show [repo_root]`
- `kanban config get <key> [repo_root]`
- `kanban config set <key> <value> [repo_root]`
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
- `kanban completion help`
- `kanban validate [repo_root]`
- `kanban doctor [repo_root]`
- `kanban doctor help`

## Repository configuration

Run `kanban init` once per repository. This creates `.kanban/` in the git root with:

- `paths.json` for backlog and sprint file locations, defaulting to `delivery/backlog` and `delivery/sprints`
- `theme.json` for terminal color behavior
- `story-points.json` for allowed values and alias conversion

If `.kanban/` is missing, operational commands fail with a prompt to run `kanban init`.

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

### Note on direnv

`.envrc` is evaluated as bash by direnv, so `eval "$(kanban completion zsh)"` cannot be
placed there — the zsh-specific completion syntax will fail. The `eval` approach in
`~/.zshrc` is the recommended setup; it runs once per shell regardless of directory.

- `crates/core`: shared parsing and validation core
- `crates/cli`: CLI interface for inspection and lightweight write flows
- `crates/tui`: reserved for the terminal UI
- `../kanban-web`: local web interface launched by the CLI

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
`docs/superpowers/specs/2026-06-03-kanban-json-output-design.md` (repo-root-relative).
