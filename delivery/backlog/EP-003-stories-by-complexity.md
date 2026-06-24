# EP-003 Codebase Hardening — Stories by Complexity

Reference for delegating EP-003 stories to suitable agents in new sessions.
Each story lives under `delivery/backlog/US-0XX-*.md` with full acceptance
criteria, NFRs, technical notes, and dependencies. Epic:
`delivery/backlog/EP-003-codebase-hardening-and-architecture-remediation.md`.

## Sequencing notes

- Critical security/data-integrity fixes (US-008..US-013) should land first.
- US-021 (split web-server module) before US-022 (move helpers to core).
- US-036 (Unicode slugify) before US-041 (consolidate slugify).
- US-025 (typed error enum) before US-024 (semantic exit codes) and pairs
  with US-020 (shared orchestration).
- US-008 (path containment) before US-009, US-017, US-026.

## high (4 stories)

Experienced developer agent; touch many call sites, need careful review.

| Story | Title | Points | File |
| ----- | ----- | ------ | ---- |
| US-008 | Path containment for task_file frontmatter and doctor writes | 8 | `US-008-path-containment-for-task-file-and-doctor-writes.md` |
| US-010 | Pin and verify the kanban upgrade install script | 8 | `US-010-pin-and-verify-upgrade-install-script.md` |
| US-013 | Advisory file lock for read-modify-write sequences | 8 | `US-013-advisory-file-lock-for-read-modify-write-sequences.md` |
| US-025 | Typed KanbanError enum at the core boundary | 8 | `US-025-typed-kanban-error-enum-at-core-boundary.md` |

## medium (16 stories)

General or developer agent; one focused PR each; some benefit from sequencing.

| Story | Title | Points | File | Depends on |
| ----- | ----- | ------ | ---- | ---------- |
| US-009 | Disable symlink following in the repository walk | 5 | `US-009-disable-symlink-following-in-repository-walk.md` | US-008 |
| US-011 | Detect EOF and TTY in interactive prompts | 5 | `US-011-detect-eof-and-tty-in-interactive-prompts.md` | - |
| US-012 | Atomic markdown writes via temp file and rename | 5 | `US-012-atomic-markdown-writes-via-temp-file-and-rename.md` | - |
| US-014 | CSRF protection for web-server mutation endpoints | 5 | `US-014-csrf-protection-for-web-server-mutation-endpoints.md` | - |
| US-015 | Verify process identity before signalling in web stop | 5 | `US-015-verify-process-identity-before-signalling-in-web-stop.md` | - |
| US-018 | Single source of truth for story and task status names | 5 | `US-018-single-source-of-truth-for-status-names.md` | US-019 |
| US-019 | Round-trip tests for shell completion script enhancement | 5 | `US-019-round-trip-tests-for-completion-enhancement.md` | US-018 |
| US-020 | Extract shared CLI orchestration for human and JSON output paths | 5 | `US-020-extract-shared-cli-orchestration-for-human-and-json-paths.md` | US-025 |
| US-021 | Split the web-server god-module into focused modules | 5 | `US-021-split-web-server-god-module-into-focused-modules.md` | US-022 |
| US-022 | Move web-server markdown mutation helpers into kanban-core | 5 | `US-022-move-markdown-mutation-helpers-into-core.md` | US-021, US-012, US-041 |
| US-023 | Move blocking I/O off the web-server async runtime | 5 | `US-023-move-blocking-io-off-async-runtime.md` | US-021 |
| US-026 | Expand validate.rs to catch writer-produced corruption | 5 | `US-026-expand-validate-to-catch-writer-corruption.md` | US-008 |

## low (8 stories)

General agent; one focused PR each.

| Story | Title | Points | File | Depends on |
| ----- | ----- | ------ | ---- | ---------- |
| US-024 | Semantic exit codes derived from KanbanErrorCode | 3 | `US-024-semantic-exit-codes-from-kanban-error-code.md` | US-025 |
| US-029 | Fix web start port TOCTOU | 3 | `US-029-fix-web-start-port-toctou.md` | - |
| US-030 | Stop swallowing missing .kanban in test cfg | 3 | `US-030-stop-swallowing-missing-kanban-in-test-cfg.md` | - |
| US-032 | Secure tempfile for the uninstall script | 3 | `US-032-secure-tempfile-for-uninstall-script.md` | - |
| US-034 | Static asset caching and Range headers | 3 | `US-034-static-asset-caching-and-range-headers.md` | - |
| US-038 | Replace per-module glob imports with explicit imports | 3 | `US-038-replace-per-module-glob-imports-with-explicit-imports.md` | - |
| US-041 | Consolidate relative_path and slugify variants | 3 | `US-041-consolidate-relative-path-and-slugify-variants.md` | US-036, US-022 |

## simple (10 stories)

Fast agent; quick cleanup PRs.

| Story | Title | Points | File | Depends on |
| ----- | ----- | ------ | ---- | ---------- |
| US-016 | Stop leaking absolute filesystem paths in web error responses | 2 | `US-016-stop-leaking-absolute-paths-in-web-error-responses.md` | - |
| US-017 | Validate sprint name segment before filesystem join | 2 | `US-017-validate-sprint-name-segment-before-filesystem-join.md` | US-008 |
| US-027 | Replace unwrap on user-controlled parent paths | 2 | `US-027-replace-unwrap-on-user-controlled-parent-paths.md` | - |
| US-033 | Avatar endpoint nosniff and image-only MIME | 2 | `US-033-avatar-endpoint-nosniff-and-image-only-mime.md` | - |
| US-035 | SSE subscriber cap | 2 | `US-035-sse-subscriber-cap.md` | - |
| US-036 | Unicode-safe slugify | 2 | `US-036-unicode-safe-slugify.md` | US-041 |
| US-037 | Warn on corrupt settings.json in theme resolution | 2 | `US-037-warn-on-corrupt-settings-json-in-theme-resolution.md` | - |
| US-028 | Remove expect in find_story_with_source | 1 | `US-028-remove-expect-in-find-story-with-source.md` | - |
| US-031 | Serialize self_manage environment-variable mutation tests | 1 | `US-031-serialize-self-manage-env-var-mutation-tests.md` | - |
| US-039 | Delete dead inject_bash_story_update completion code | 1 | `US-039-delete-dead-inject-bash-story-update-code.md` | - |
| US-040 | Use from_utf8_lossy in completion generation | 1 | `US-040-use-from-utf8-lossy-in-completion-generation.md` | - |

## Totals

- 38 stories (US-008..US-041)
- 4 high, 12 medium, 7 low, 10 simple (counts per the table above; some
  cross-listed dependencies are not duplicates)
- 174 story points total

## Verification commands (AGENTS.md)

Run from repo root after each story:

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build`
- `kanban validate .`
- `kanban doctor .`
- Bump `workspace.package.version` in `Cargo.toml` per the AGENTS.md SemVer
  scheme for every merged story.
