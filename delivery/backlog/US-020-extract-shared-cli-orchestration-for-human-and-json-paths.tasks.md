# Tasks for US-020

Parent User Story: US-020
Sprint: ~

## TASK-US-020-001 - Extract shared story-list scope resolver

Status: done
Tags: cli, refactor

Description:
Added cli::ops::StoryListScope + resolve_story_list_scope; both human and JSON paths now share one scope-resolution implementation while formatting labels separately.

## TASK-US-020-002 - Extract shared sprint-create input builder

Status: done
Tags: cli, refactor

Description:
Added cli::ops::build_create_sprint_input_from_flags; both human and JSON paths now share parsing/default logic for --number/--headline/--start/--end.

## TASK-US-020-003 - Rewire main.rs and json_out.rs

Status: done
Tags: cli, json

Description:
main.rs now uses human_label(); json_out.rs uses json_label() with backward-compatible default/current scope. Existing integration tests pass unchanged.

## TASK-US-020-004 - Verify and bump version to 26.6.2412

Status: done
Tags: verify

Description:
357 tests pass; fmt/clippy/build/validate/doctor clean.
