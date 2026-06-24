# Tasks for US-023

Parent User Story: US-023
Sprint: ~

## TASK-US-023-001 - Wrap blocking handlers in spawn_blocking

Status: done
Tags: web-server, async

Description:
Repository snapshot, metrics source load, config load, team load, story/epic detail load, and all mutation handlers now execute blocking core/fs/subprocess work inside a shared run_blocking helper.

## TASK-US-023-002 - Cache git branch in app state

Status: done
Tags: web-server, cache

Description:
Added branch_cache to AppState and cached_git_branch helper. File watcher now invalidates the cache on each event before broadcasting change notifications.

## TASK-US-023-003 - Add branch-cache test

Status: done
Tags: test

Description:
Handler-level async test proves cached_git_branch returns the cached value without touching git/repo state.

## TASK-US-023-004 - Verify and bump version to 26.6.2414

Status: done
Tags: verify

Description:
Full workspace fmt/clippy/test/build/validate/doctor clean after async offload changes.
