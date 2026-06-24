# Tasks for US-026

Parent User Story: US-026
Sprint: ~

## TASK-US-026-001 - Add validate_story_with_config accepting &KanbanConfig

Status: done
Tags: core, validate

Description:
New function takes Option<&KanbanConfig> to avoid load_kanban_config(parent) rediscovery. validate_story delegates with None. validate_repository passes Some(&config).

## TASK-US-026-002 - Add duplicate-story-id rule

Status: done
Tags: core, validate

Description:
Collects IDs across all stories in validate_repository; reports duplicate-story-id for each file sharing an ID.

## TASK-US-026-003 - Add out-of-tree-story-path rule

Status: done
Tags: core, validate

Description:
In validate_story_with_config: canonicalizes story file_path and backlog_path, reports if story canonical path doesn't start with backlog root.

## TASK-US-026-004 - Add tests for duplicate-ID and out-of-tree

Status: done
Tags: test

Description:
validate_repository_reports_duplicate_story_ids: two files same ID, asserts 2 issues. validate_repository_reports_out_of_tree_story_path: story outside backlog, asserts rule fires.

## TASK-US-026-005 - Verify and bump version to 26.6.2409

Status: done
Tags: verify

Description:
353 tests pass; fmt/clippy/build/validate/doctor clean.
