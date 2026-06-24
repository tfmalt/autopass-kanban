# Tasks for US-019

Parent User Story: US-019
Sprint: ~

## TASK-US-019-001 - Add zsh helper wiring tests

Status: done
Tags: test

Description:
4 tests: sprint_names wired to sprint show, story_ids wired to story commands, status/task helpers wired, epic/phase/config helpers wired. assert_zsh_helper_wired checks helper appears on argument line not just function def.

## TASK-US-019-002 - Add bash injection marker tests

Status: done
Tags: test

Description:
8 tests: story_move, task_status (both add+update), doctor_fix_target, sprint_create, story_plan, config, story_update, phase_show, task_delete. Each asserts unique marker from inject_bash_* replacement is present.

## TASK-US-019-003 - Add regression detection test

Status: done
Tags: test

Description:
bash_completion_no_remaining_default_case_blocks verifies story move block contains dynamic lookup, catching silent no-op regressions.

## TASK-US-019-004 - Verify and bump version to 26.6.2408

Status: done
Tags: verify

Description:
351 tests pass; fmt/clippy/build/validate/doctor clean.
