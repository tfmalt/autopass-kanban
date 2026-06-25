# Tasks for US-028

Parent User Story: US-028
Sprint: ~

## TASK-US-028-001 - Remove expect in find_story_with_source

Status: done
Tags: core, robustness

Description:
Replaced the same-scan expect() with a propagated anyhow error naming the vanished story, preserving Option(None) for not-found and eliminating the panic path.
