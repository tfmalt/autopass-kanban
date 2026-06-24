# Tasks for US-010

Parent User Story: US-010
Sprint: ~

## TASK-US-010-001 - Pin install script to release tag + SHA-256 verify

Status: done
Tags: cli, security

Description:
Pinned URL to v<version>/scripts/install.sh; hard-refuse main; fetch install.sh.sha256 and verify before sh; gated GITHUB_LATEST_TAG/GITHUB_API_BASE behind cfg(test)/KANBAN_ALLOW_UNSAFE_OVERRIDE.

## TASK-US-010-002 - ADR-002 + tests

Status: done
Tags: docs, tests

Description:
Wrote ADR-002 upgrade trust root; tests for pinned URL, checksum match/mismatch, parse_checksum_asset, ensure_pinned_tag, env gating.

## TASK-US-010-003 - Verify upgrade against a real published release

Status: todo
Tags: cli, release

Description:
AC#4 unverified: requires a published GitHub release with install.sh.sha256 asset and network. Cannot be unit-tested; needs integration verification once EP-002 publishes the checksum asset.
