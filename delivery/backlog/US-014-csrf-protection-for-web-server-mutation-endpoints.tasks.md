# Tasks for US-014

Parent User Story: US-014
Sprint: ~

## TASK-US-014-001 - Add csrf_guard middleware

Status: done
Tags: web-server, security

Description:
axum from_fn_with_state middleware: passes safe methods (GET/HEAD/OPTIONS); requires Origin (or Referer fallback) authority matching bound host:port for mutations; 403 on mismatch/absent. authority_from_origin_or_referer requires scheme.

## TASK-US-014-002 - Wire middleware into router

Status: done
Tags: web-server

Description:
Applied middleware::from_fn_with_state(csrf_guard) layer to the serve() router so all routes are covered.

## TASK-US-014-003 - Add CSRF tests and tower dev-dep

Status: done
Tags: test

Description:
6 tests: same-origin allow, cross-origin reject, missing origin/referer reject, referer fallback allow, GET passthrough, authority parser. Added tower dev-dependency for oneshot.

## TASK-US-014-004 - Verify and bump version to 26.6.2405

Status: done
Tags: verify

Description:
All ACs verified; 332 tests pass; fmt/clippy/build/validate/doctor clean.
