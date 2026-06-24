# Tasks for US-018

Parent User Story: US-018
Sprint: ~

## TASK-US-018-001 - Make status consts pub in constants.rs

Status: done
Tags: core

Description:
CANONICAL_STORY_STATUSES, CANONICAL_TASK_STATUSES, SPRINT_STATUS_DISPLAY_ORDER changed from pub(crate) to pub so CLI consumers can reference them.

## TASK-US-018-002 - Replace bare-literal status arrays across consumers

Status: done
Tags: refactor

Description:
Replaced: web-server BOARD_STATUSES, render/story.rs status_order+task loop, render/sprint.rs, render/epic.rs, render/phase.rs phase_status_display_order, doctor.rs matches!. Fixed divergent lists missing backlog/blocked (bugs).

## TASK-US-018-003 - Generate completion status lists from consts

Status: done
Tags: completion

Description:
ZSH: token replacement __KANBAN_STORY_STATUSES__/__KANBAN_TASK_STATUSES__ with const-derived lines. Bash: same tokens replaced with const.join(' '). All 5 sites covered.

## TASK-US-018-004 - Add consistency test

Status: done
Tags: test

Description:
completion_status_lists_match_canonical_consts verifies bash/zsh output contains exactly CANONICAL_STORY_STATUSES and CANONICAL_TASK_STATUSES.

## TASK-US-018-005 - Verify and bump version to 26.6.2407

Status: done
Tags: verify

Description:
337 tests pass; fmt/clippy/build/validate/doctor clean. Fixed 2 divergent-list bugs (missing backlog, missing blocked).
