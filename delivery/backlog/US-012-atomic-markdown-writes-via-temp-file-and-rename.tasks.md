# Tasks for US-012

Parent User Story: US-012
Sprint: ~

## TASK-US-012-001 - Add atomic_write helper in core

Status: done
Tags: core, write-safety

Description:
repository::atomic_write uses tempfile::NamedTempFile::new_in(parent) + sync_all + persist; promotes tempfile to core runtime dep. Creates parent dir if missing.

## TASK-US-012-002 - Replace fs::write in all production writers

Status: done
Tags: refactor

Description:
Replaced 16 sites: story.rs(6), sprint.rs(4), doctor.rs(3), markdown.rs(1), epic.rs(1), config.rs(1), web-server/lib.rs(2). Test fixtures untouched.

## TASK-US-012-003 - Add crash-simulation and durability tests

Status: done
Tags: test

Description:
3 tests: success replace, failed persist leaves original intact + no partial at target, parent-dir creation. Verified all ACs.

## TASK-US-012-004 - Verify and bump version to 26.6.2404

Status: done
Tags: verify

Description:
cargo fmt/test/clippy/build pass; validate/doctor clean. Version bumped.
