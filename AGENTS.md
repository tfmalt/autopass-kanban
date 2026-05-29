# Kanban Tool Agent Guide

This directory contains the Rust workspace for the markdown-first backlog tooling.
These instructions apply to all files under `tools/kanban/`.

## Scope

- `crates/core` contains shared parsing, domain logic, validation, and write helpers.
- `crates/cli` contains the `kanban` CLI and the documented `kb` alias in help/docs.
- `crates/tui` is reserved for a terminal UI.
- `crates/web` is reserved for a local web interface.
- `target/` is build output and must not be edited manually or committed.

## Development Rules

- Keep backlog semantics in `crates/core`; keep CLI code focused on argument parsing, output, and command orchestration.
- Preserve the repository's markdown backlog as the source of truth. Do not introduce a database, generated state store, or hidden metadata cache unless an ADR or explicit user decision requires it.
- Prefer small, explicit Rust types for parsed backlog concepts instead of passing loosely structured strings through the codebase.
- Keep command behavior deterministic and safe for human-edited markdown files.
- Do not silently rewrite unrelated frontmatter, prose, ordering, or formatting in backlog documents.
- Use full local ISO 8601 timestamps with numeric timezone offset when writing backlog lifecycle fields.
- Avoid adding new CLI command names, status names, or file layout conventions without checking `doc/backlog/README.md` and the relevant backlog-board workflow.

## Versioning

- The kanban workspace version is defined in `tools/kanban/Cargo.toml` under `[workspace.package]`.
- Always update the version when finished with a task.
- Use this SemVer scheme:
  - `MAJOR` is the last two digits of the current year, for example `26` for 2026.
  - `MINOR` is the current month without a leading zero, for example `1`, `5`, or `12`.
  - `PATCH` is the current day of month without a leading zero followed by the update count for that day. For example, the 3rd update on 5 May is `53`, and the 12th update on 29 May is `2912`.

## Verification

Run commands from `tools/kanban/` unless noted otherwise:

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- After all tests pass for a completed change, always run `cargo build` before reporting the work finished.

For changes that modify markdown parsing or writing behavior, also run:

- `cargo run -p kanban-cli -- validate ../..`
- `cargo run -p kanban-cli -- doctor ../..`

If a command cannot be run, report the reason and what remains unverified.
