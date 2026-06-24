# Tasks for US-011

Parent User Story: US-011
Sprint: ~

## TASK-US-011-001 - Detect EOF in prompt helpers

Status: done
Tags: core, prompt

Description:
prompt/read_prompted_line bail on read_line Ok(0) with actionable message; fixes doctor auto-apply and Choice busy-loop. Unit tests added.

## TASK-US-011-002 - Reject empty sprint headline

Status: done
Tags: cli, sprint

Description:
prompt_create_sprint validates non-empty headline; core slug check already guards before write.

## TASK-US-011-003 - Add doctor fix --non-interactive

Status: done
Tags: cli, doctor

Description:
DoctorCommand::Fix gains --non-interactive; run_doctor_fix_non_interactive applies auto/no-prompt fixes and skips guided/manual with summary.

## TASK-US-011-004 - Update completion injection for new flag

Status: done
Tags: cli, completion

Description:
inject_bash_doctor_fix_target opts line includes --non-interactive; added parser test.

## TASK-US-011-005 - Verify EOF integration and bump version

Status: done
Tags: verify

Description:
Verified doctor fix/sprint create < /dev/null exit non-zero with no writes; non-interactive path verified. Version 26.6.2403.
