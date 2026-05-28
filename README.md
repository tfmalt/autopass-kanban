# Kanban

Rust workspace for the markdown-first backlog tooling.

CLI binary name:
- `kanban`
- short alias in help/docs: `kb`

Implemented read-only commands:
- `kanban sprint current [repo_root]`
- `kanban sprint list [repo_root]`
- `kanban sprint show <name> [repo_root]`
- `kanban phase show <phase> [repo_root]`
- `kanban story show <id> [repo_root]`
- `kanban validate [repo_root]`
- `kanban doctor [repo_root]`

- `crates/core`: shared parsing and validation core
- `crates/cli`: initial read-only CLI interface
- `crates/tui`: reserved for the terminal UI
- `crates/web`: reserved for the local web interface

Run tests with `cargo test` from this directory.
