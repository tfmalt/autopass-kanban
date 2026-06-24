# Tasks for US-008

Parent User Story: US-008
Sprint: ~

## TASK-US-008-001 - Add ensure_path_inside containment helper

Status: done
Tags: core, security

Description:
Added ensure_path_inside + validate_task_file_frontmatter_value in util.rs; threaded through read_story_file (task_file reads) and apply_doctor_fix (writes); added invalid-task-file-path validate rule.

## TASK-US-008-002 - Traversal and symlink escape tests

Status: done
Tags: tests

Description:
Covered .. traversal, absolute/separator task_file, symlinked task_file escape, and doctor out-of-tree write refusal.
