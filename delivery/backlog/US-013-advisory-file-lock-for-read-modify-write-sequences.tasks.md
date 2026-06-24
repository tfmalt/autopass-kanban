# Tasks for US-013

Parent User Story: US-013
Sprint: ~

## TASK-US-013-001 - RepoLock advisory per-repo lock

Status: done
Tags: core, concurrency

Description:
Added lock.rs with fs4 exclusive lock, 5s blocking timeout; threaded through story/epic/doctor mutation entry points.

## TASK-US-013-002 - Web-server write mutex + ADR-001

Status: done
Tags: web, concurrency

Description:
Added AppState::write_lock tokio Mutex guarding all mutation handlers; documented model in ADR-001 and AGENTS.md; added concurrency + early-return tests.
