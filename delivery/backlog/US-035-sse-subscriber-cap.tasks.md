# Tasks for US-035

Parent User Story: US-035
Sprint: ~

## TASK-US-035-001 - Cap SSE subscribers at 64

Status: done
Tags: web-server, reliability

Description:
Added AppState subscriber counter, guard-based decrement, and 503 rejection when the SSE cap is exceeded.
