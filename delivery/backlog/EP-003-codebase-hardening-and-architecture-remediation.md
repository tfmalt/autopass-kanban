---
id: EP-003
type: epic
status: in-progress
phase: 1
owner: Thomas Malt / Tooling Lead
milestone: MP2
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T17:30:00+0200
---

# Epic: Codebase hardening and architecture remediation

---

---

## Business Context

A principal-engineer review of the `autopass-kanban` Rust workspace (~27k lines
across `crates/core`, `crates/cli`, and `crates/web-server`) surfaced security,
crash-safety, concurrency, and maintainability issues that are exploitable by a
local attacker or a malicious web page today, and that allow the markdown writers
to corrupt user data on crash or under concurrency. The review also identified
significant code duplication — most importantly markdown-mutation logic split
between `core` and `web-server` with divergent semantics — that violates the
AGENTS.md rule "Keep backlog semantics in `crates/core`" and lets the web UI
write markdown the CLI validator rejects.

This Epic consolidates the remediation into individually implementable User
Stories, each marked with a complexity (`simple`, `low`, `medium`, `high`) so
work can be delegated to the most suitable agent. The stories are sequenced so
security and data-integrity fixes land first, followed by architecture and
deduplication work.

---

## Business Value

- **Primary benefit:** The markdown source of truth cannot be corrupted by path
  traversal, symlink planting, crash-mid-write, or concurrent CLI + web server
  access. The local web server cannot be driven by a cross-origin page, leak
  absolute filesystem paths, or write outside the backlog root.
- **Secondary benefit:** A single markdown-mutation implementation shared
  between the CLI and web server eliminates the divergence where the web UI can
  write files `kanban validate` rejects. Status names, slugify helpers, and
  orchestration logic each have one source of truth.
- **Risk if not done:** A compromised `main` branch yields RCE on every
  `kanban upgrade` user; a planted symlink or `task_file: ../../` value writes
  outside the backlog; a SIGTERM mid-write truncates a story file; two
  concurrent `story move` calls silently lose one update.

---

## Users and Stakeholders

| Role                        | Involvement                                                          |
| --------------------------- | -------------------------------------------------------------------- |
| Developer running `kanban`  | Expects safe, non-destructive writes and predictable exit codes      |
| Developer running `kanban web start` | Expects the local server to resist cross-origin and local attacks |
| Maintainer adding a status/command | Expects a single source of truth so additions cannot drift     |
| AI agent implementing a story | Uses the complexity tag to pick the right agent and effort         |
| Tooling lead                | Owns the trust root for `kanban upgrade` and the concurrency model   |

---

## Scope

### In Scope

- Filesystem path containment for every `task_file`, doctor, and web-server
  write, plus a `validate` rule rejecting unsafe `task_file` values.
- Atomic markdown writes (temp file + `fsync` + rename) across all writers.
- Advisory file locking around read-modify-write sequences shared by the CLI
  and the web server.
- Pinning and verifying the `kanban upgrade` install script to a release tag
  with a checksum from a second trusted channel.
- TTY/EOF detection in interactive prompts so `doctor fix < /dev/null` cannot
  auto-apply fixes and `sprint create` cannot produce empty headlines.
- Web-server hardening: `Origin`/`Host` allow-list for mutations, generic error
  responses, sprint-name validation, avatar `nosniff` + image-only MIME, SSE
  subscriber cap, static-asset caching, blocking-I/O moved off the async
  runtime.
- Stale-PID verification before `web stop` signals a process.
- Splitting the 2000-line `web-server/src/lib.rs` god-module and moving the
  duplicated markdown mutation helpers into `crates/core`.
- A single source of truth for story/task status names, slugify variants,
  `relative_path`, and shared CLI orchestration (human vs JSON paths).
- Typed `KanbanError` enum at the core boundary to replace string-sniffing
  error classification, and semantic exit codes derived from it.
- Expanding `validate.rs` to catch corruption the writer can produce
  (duplicate IDs, out-of-tree paths, unsafe `task_file`).
- Smaller cleanups: replace `unwrap()`/`expect()` on user data, fix CRLF/LF
  consistency, delete dead code, replace glob imports.

### Out of Scope

- Replacing `anyhow` entirely — it stays as the internal error model; only the
  public `core` boundary gains a typed enum.
- A full async rewrite of `kanban-core`; blocking I/O concerns are limited to
  the web-server handlers.
- Code signing / Sigstore for release artifacts (that belongs with EP-002).
- Adding new CLI commands, status names, or file-layout conventions.

---

## Acceptance Criteria

- [ ] No `fs::write`/`fs::remove_file`/`fs::rename` in `core` or `web-server`
      writes to a path outside the canonicalized backlog root, verified by a
      shared `ensure_path_inside` helper and tests with symlinks and `..`
      `task_file` values.
- [ ] Every markdown writer uses an atomic temp-file-then-rename pattern and a
      test demonstrates a simulated mid-write crash leaves the original file
      intact.
- [ ] Concurrent `story move` calls on the same story serialize via an advisory
      lock; a test demonstrates no silent update loss.
- [ ] `kanban upgrade` fetches the install script from a pinned release tag and
      verifies a SHA-256 before invoking `sh`.
- [ ] `kanban doctor fix < /dev/null` and `kanban sprint create < /dev/null`
      exit non-zero with a clear "input closed" message instead of auto-applying
      or producing empty data.
- [ ] The web server rejects non-GET requests whose `Origin`/`Host` does not
      match the bound address.
- [ ] Web error responses do not contain absolute filesystem paths.
- [ ] `crates/web-server/src/lib.rs` no longer contains
      `replace_markdown_body`/`replace_frontmatter_fields`/
      `replace_section_content`/`replace_sprint_title`; the web server calls
      `kanban_core::markdown` for all mutations.
- [ ] Story and task status names appear in exactly one `const &[&str]` each,
      consumed by clap, completion, theme, render, and validate.
- [ ] `KanbanErrorCode::classify` no longer inspects `error.to_string()`
      substrings; it matches on a typed `KanbanError` enum.
- [ ] `cargo fmt --all -- --check`, `cargo test`,
      `cargo clippy --workspace --all-targets -- -D warnings`, and
      `cargo build` pass.
- [ ] `kanban validate .` and `kanban doctor .` pass.

---

## Non-Functional Requirements

| Area                        | Requirement                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------ |
| **Security**                | No writer escapes the backlog root; no cross-origin mutation; no unsigned code execution   |
| **Data integrity**          | Writers are atomic and crash-safe; concurrent writers serialize                            |
| **Maintainability**         | Status names, slugify, markdown mutation, and CLI orchestration each have one source       |
| **Observability**           | Exit codes distinguish bad-input from not-found from not-initialized                       |
| **Backward compatibility**  | No new CLI command names, status names, or file-layout conventions introduced              |

---

## Architecture Considerations

- **Relevant architecture principles:** Markdown remains the source of truth;
  the remediation adds safety around how it is written, not a new state store.
  The AGENTS.md rule "Keep backlog semantics in `crates/core`" is enforced by
  moving the duplicated web-server markdown helpers into `core`.
- **Key patterns in play:** canonicalize-then-`starts_with` path containment;
  `tempfile::NamedTempFile::persist` for atomic writes; advisory file locking
  for read-modify-write; typed error enum at the crate boundary;
  `spawn_blocking` for synchronous I/O in async handlers.
- **ADR references:** None yet. The concurrency model (single-writer vs
  advisory lock) and the upgrade trust root (pinned tag + checksum) should each
  get an ADR.
- **Known risks or constraints:** Atomic rename on Windows is same-volume only;
  the temp file must live in the target directory. Advisory locks are not
  enforced across processes that bypass the helper, so document the model.

---

## Dependencies

| Dependency                       | Type           | Status    | Notes                                                              |
| -------------------------------- | -------------- | --------- | ------------------------------------------------------------------ |
| EP-001                           | Epic           | Done      | Installability foundation touched by the upgrade-trust story       |
| EP-002                           | Epic           | In Progress | GitHub release distribution feeds the pinned upgrade flow          |
| `tempfile` crate                 | Dependency     | Available | Already a workspace dev-dependency; promote to `core` runtime      |

---

## Child User Stories

| Story ID | Title                                                                                      | Complexity | Points |
| -------- | ------------------------------------------------------------------------------------------ | ---------- | ------ |
| US-008   | Path containment for task_file frontmatter and doctor writes                              | high       | 8      |
| US-009   | Disable symlink following in the repository walk                                           | medium     | 5      |
| US-010   | Pin and verify the kanban upgrade install script                                           | high       | 8      |
| US-011   | Detect EOF and TTY in interactive prompts                                                  | medium     | 5      |
| US-012   | Atomic markdown writes via temp file and rename                                            | medium     | 5      |
| US-013   | Advisory file lock for read-modify-write sequences                                         | high       | 8      |
| US-014   | CSRF protection for web-server mutation endpoints                                          | medium     | 5      |
| US-015   | Verify process identity before signalling in web stop                                      | medium     | 5      |
| US-016   | Stop leaking absolute filesystem paths in web error responses                              | simple     | 2      |
| US-017   | Validate sprint name segment before filesystem join                                        | simple     | 2      |
| US-018   | Single source of truth for story and task status names                                     | medium     | 5      |
| US-019   | Round-trip tests for shell completion script enhancement                                   | medium     | 5      |
| US-020   | Extract shared CLI orchestration for human and JSON output paths                           | medium     | 5      |
| US-021   | Split the web-server god-module into focused modules                                       | medium     | 5      |
| US-022   | Move web-server markdown mutation helpers into kanban-core                                 | medium     | 5      |
| US-023   | Move blocking I/O off the web-server async runtime                                         | medium     | 5      |
| US-024   | Semantic exit codes derived from KanbanErrorCode                                           | low        | 3      |
| US-025   | Typed KanbanError enum at the core boundary                                                | high       | 8      |
| US-026   | Expand validate.rs to catch writer-produced corruption                                     | medium     | 5      |
| US-027   | Replace unwrap on user-controlled parent paths                                             | simple     | 2      |
| US-028   | Remove expect in find_story_with_source                                                    | simple     | 1      |
| US-029   | Fix web start port TOCTOU                                                                  | low        | 3      |
| US-030   | Stop swallowing missing .kanban in test cfg                                                | low        | 3      |
| US-031   | Serialize self_manage environment-variable mutation tests                                  | simple     | 1      |
| US-032   | Secure tempfile for the uninstall script                                                   | low        | 3      |
| US-033   | Avatar endpoint nosniff and image-only MIME                                                | simple     | 2      |
| US-034   | Static asset caching and Range headers                                                     | low        | 3      |
| US-035   | SSE subscriber cap                                                                         | simple     | 2      |
| US-036   | Unicode-safe slugify                                                                       | simple     | 2      |
| US-037   | Warn on corrupt settings.json in theme resolution                                          | simple     | 2      |
| US-038   | Replace per-module glob imports with explicit imports                                      | low        | 3      |
| US-039   | Delete dead inject_bash_story_update completion code                                       | simple     | 1      |
| US-040   | Use from_utf8_lossy in completion generation                                               | simple     | 1      |
| US-041   | Consolidate relative_path and slugify variants                                             | low        | 3      |

---

## Definition of Done (Epic Level)

- [ ] All child User Stories are complete and accepted
- [ ] No `fs::write` in `core`/`web-server` bypasses path containment and
      atomic-write helpers (verified by grep + tests)
- [ ] The web server shares markdown mutation logic with the CLI through
      `kanban_core::markdown`
- [ ] Exit codes and JSON error codes derive from a typed enum, not strings
- [ ] `cargo fmt --all -- --check`, `cargo test`,
      `cargo clippy --workspace --all-targets -- -D warnings`, and
      `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml` per `AGENTS.md` for every merged
      child story
- [ ] ADRs recorded for the concurrency model and the upgrade trust root

---

## Notes and Open Questions

| #   | Question / Assumption                                                                  | Owner        | Due        | Resolved |
| --- | -------------------------------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should the advisory lock be per-repo or per-sprint?                                    | Tooling lead | 2026-07-04 | Yes — per-repo (`.kanban/.lock`) |
| 2   | Should the upgrade checksum be embedded in the binary or fetched from a second host?   | Tooling lead | 2026-07-04 | Yes — fetched from the same release tag's assets as `install.sh.sha256` |
| 3   | Is a `KanbanError` enum per-public-function feasible, or one shared enum for the crate? | Tooling lead | 2026-07-04 | Yes — one shared `KanbanError` enum for the crate |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic Epic template derived from the kanban tooling conventions_
