# Kanban Tool Agent Guide

This repository contains the Rust workspace for the markdown-first backlog tooling.
These instructions apply to all files in this repository.

## Scope

- `crates/core` contains shared parsing, domain logic, validation, and write helpers.
- `crates/cli` contains the `kanban` CLI and the documented `kb` alias in help/docs.
- The local web app source lives in `web/`; production `kanban web start` launches the embedded Rust web server from `crates/web-server`.
- `target/` is build output and must not be edited manually or committed.

## Development Rules

- Keep backlog semantics in `crates/core`; keep CLI code focused on argument parsing, output, and command orchestration.
- Preserve the repository's markdown backlog as the source of truth. Do not introduce a database, generated state store, or hidden metadata cache unless an ADR or explicit user decision requires it.
- Prefer small, explicit Rust types for parsed backlog concepts instead of passing loosely structured strings through the codebase.
- Keep command behavior deterministic and safe for human-edited markdown files.
- Do not silently rewrite unrelated frontmatter, prose, ordering, or formatting in backlog documents.
- Use full local ISO 8601 timestamps with numeric timezone offset when writing backlog lifecycle fields.
- Avoid adding new CLI command names, status names, or file layout conventions without checking how existing commands and the configured backlog directory use those names in practice.

## Concurrency and write safety

- The markdown backlog is a shared source of truth that can be mutated by the CLI and the local web server at the same time. Every public mutation in `crates/core` acquires an advisory, per-repo exclusive lock (`.kanban/.lock`, 5 s blocking timeout) for the duration of its read-modify-write sequence. See `delivery/decisions/ADR-001`.
- The lock is advisory: processes that bypass the `RepoLock` helper (editors, git, scripts) are not stopped. When adding a new mutation entry point in `crates/core`, wrap it with `RepoLock::acquire(&config.repo_root)?` before the first write.
- Web-server mutation handlers additionally serialize through `AppState::write_lock` (`tokio::sync::Mutex`).
- Writers must keep user-derived paths inside the canonicalized backlog root using `ensure_path_inside` (see US-008). `task_file` frontmatter is validated by `validate_task_file_frontmatter_value` and bounded on read and write.

## Versioning

- The kanban workspace version is defined in `Cargo.toml` under `[workspace.package]`.
- Always update the version when finished with a task.
- Use this SemVer scheme:
  - `MAJOR` is the last two digits of the current year, for example `26` for 2026.
  - `MINOR` is the current month without a leading zero, for example `1`, `5`, or `12`.
  - `PATCH` is the current day of month without a leading zero followed by the update count for that day, padded to two digits for counts 1 through 9. For example, the 4th update on 31 May is `3104`, the 4th update on 1 June is `104`, and the 12th update on 1 June is `112`.

## Verification

Run commands from the repository root unless noted otherwise:

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- After all tests pass for a completed change, always run `cargo build` before reporting the work finished.

For changes that modify markdown parsing or writing behavior, also run:

- `cargo run -p kanban-cli -- validate .`
- `cargo run -p kanban-cli -- doctor .`

The `.` argument tells the CLI to use the current directory as the target repository root, reading backlog configuration from `.kanban/settings.json`. To verify against a different backlog, replace `.` with the path to that repository's root.

If a command cannot be run, report the reason and what remains unverified.
