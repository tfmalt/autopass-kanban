# Tasks for US-021

Parent User Story: US-021
Sprint: ~

## TASK-US-021-001 - Extract web-server modules

Status: done
Tags: web-server, refactor

Description:
Created dto.rs, metrics.rs, snapshot.rs, sprint_io.rs, team.rs, and handlers/mod.rs. lib.rs now contains only bootstrap/state/router/middleware plus CSRF tests.

## TASK-US-021-002 - Move tests with owning modules

Status: done
Tags: test, refactor

Description:
Moved metrics, team, sprint slugify, and replace_markdown_body tests into their modules so every source file stays under the ~500-line target.

## TASK-US-021-003 - Preserve public behavior through rewire

Status: done
Tags: web-server

Description:
Router now calls handlers module functions; metrics and snapshot loaders are imported from modules. All 14 web-server tests and full workspace tests pass unchanged.

## TASK-US-021-004 - Verify and bump version to 26.6.2413

Status: done
Tags: verify

Description:
All source files under 500 lines (lib.rs 341, handlers 414, metrics 449, others lower). Full workspace fmt/clippy/test/build/validate/doctor clean.
