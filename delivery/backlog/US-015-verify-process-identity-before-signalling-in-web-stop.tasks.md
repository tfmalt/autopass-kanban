# Tasks for US-015

Parent User Story: US-015
Sprint: ~

## TASK-US-015-001 - Add process_is_kanban_web identity check

Status: done
Tags: cli, security

Description:
Unix: ps -o comm= -p {pid}, requires 'kanban' in command name. Windows: QueryFullProcessImageNameW, requires exe stem 'kanban'. Fallback platform returns false.

## TASK-US-015-002 - Integrate identity check into read_web_process_state

Status: done
Tags: cli

Description:
read_web_process_state now returns Stale (not Running) when process exists but isn't a kanban process. Handles recycled-PID in one place for stop/start/status.

## TASK-US-015-003 - Add recycled/dead PID tests

Status: done
Tags: test

Description:
4 tests: pid-zero rejection, non-kanban process rejection (spawned sleep), recycled PID -> Stale, dead PID -> Stale.

## TASK-US-015-004 - Verify and bump version to 26.6.2406

Status: done
Tags: verify

Description:
336 tests pass; fmt/clippy/build/validate/doctor clean.
