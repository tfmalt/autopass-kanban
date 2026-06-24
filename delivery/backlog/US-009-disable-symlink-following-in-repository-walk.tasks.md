# Tasks for US-009

Parent User Story: US-009
Sprint: ~

## TASK-US-009-001 - Make follow_links(false) explicit on both walks

Status: done
Tags: core, security

Description:
Added .follow_links(false) to collect_user_story_files and collect_epic_files WalkDir calls. Added explicit is_symlink() skip check.

## TASK-US-009-002 - Add canonicalized-path containment check in walk

Status: done
Tags: core, security

Description:
Both walks now canonicalize each entry path and skip files whose canonical path is outside the canonicalized backlog root.

## TASK-US-009-003 - Add symlink-planting test

Status: done
Tags: test

Description:
Unix-only test: creates a real US-001 file and a symlinked US-002 pointing outside; asserts only US-001 is collected.

## TASK-US-009-004 - Verify and bump version to 26.6.2410

Status: done
Tags: verify

Description:
354 tests pass; fmt/clippy/build/validate/doctor clean.
