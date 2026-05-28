# Kanban

Rust workspace for the markdown-first backlog tooling.

CLI binary name:
- `kanban`
- short alias in help/docs: `kb`

Implemented commands:
- `kanban sprint current [repo_root]`
- `kanban sprint list [repo_root]`
- `kanban sprint show <name> [repo_root]`
- `kanban sprint create [repo_root]`
- `kanban sprint rollover <name> [repo_root]`
- `kanban phase show <phase> [repo_root]`
- `kanban story show <id> [repo_root]`
- `kanban story move <id> <status> [-a|--assignee "Name <email>"] [repo_root]`
- `kanban task add <story_id> --title <title> --description <text> [--status <status>] [--tags <a,b>] [repo_root]`
- `kanban task update <story_id> <task_id> [--title <title>] [--description <text>] [--status <status>] [--tags <a,b>] [repo_root]`
- `kanban validate [repo_root]`
- `kanban doctor [repo_root]`

- `crates/core`: shared parsing and validation core
- `crates/cli`: CLI interface for inspection and lightweight write flows
- `crates/tui`: reserved for the terminal UI
- `crates/web`: reserved for the local web interface

Run tests with `cargo test` from this directory.
