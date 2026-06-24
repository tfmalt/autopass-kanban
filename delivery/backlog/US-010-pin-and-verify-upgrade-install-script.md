---
id: US-010
type: user-story
status: in-progress
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 8
work_started: 2026-06-24T11:17:17+0200
work_done: 
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T11:17:17+0200
---

# User Story: Pin and verify the kanban upgrade install script

---

## Story Statement

**As a** user running `kanban upgrade`,
**I want** the install script to be fetched from a pinned release tag and
verified against a checksum from a second trusted channel before execution,
**so that** a compromise of the `main` branch cannot execute arbitrary code on
my machine via the upgrade flow.

---

## Background

`crates/cli/src/self_manage.rs:8-9` fetches
`https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh`
and pipes it into `sh` with no checksum or signature. The `long_about` advertises
checksum verification, but that only covers the binary the script later
downloads — not the script itself. The only trust root is TLS to GitHub. A
compromise of `main` (or the GitHub account) yields RCE on every upgrading user.
Additionally, `resolve_latest_version` honors `GITHUB_LATEST_TAG` and
`GITHUB_API_BASE` env vars that can suppress or redirect the update check.

**Complexity: high** — requires a trust-root decision (ADR), a checksum source,
and changes to the fetch/verify/exec flow plus tests.

---

## Acceptance Criteria

**Scenario 1: Script is fetched from the pinned release tag**

```gherkin
Given `kanban upgrade` resolves latest release `v26.6.2305`
When it fetches the install script
Then the URL is `.../v26.6.2305/scripts/install.sh` (not `main`)
And the downloaded bytes are verified against a SHA-256 before execution
```

**Scenario 2: Checksum mismatch aborts before execution**

```gherkin
Given the fetched script's SHA-256 does not match the expected checksum
When `kanban upgrade` verifies it
Then it exits non-zero with a clear "checksum mismatch" message
And the script is not executed
```

**Scenario 3: Unsafe env overrides are gated**

```gherkin
Given `GITHUB_API_BASE` or `GITHUB_LATEST_TAG` is set in the environment
When `kanban upgrade` runs outside test configuration
Then it either ignores the override or requires an explicit opt-in flag
And it documents the trust implications
```

**Scenario 4: Upgrade still works against a real release**

```gherkin
Given a published GitHub release with a checksummed install script
When `kanban upgrade` runs
Then it verifies and executes the script and the binary is updated
```

---

## Non-Functional Requirements

| Area             | Requirement                                                                |
| ---------------- | -------------------------------------------------------------------------- |
| **Security**     | No execution of fetched code without checksum verification from a second channel |
| **Traceability** | The resolved tag, script URL, and checksum are logged on upgrade           |
| **Auditability** | The trust model is documented in an ADR and `AGENTS.md`                    |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`, `EP-002#acceptance-criteria`
- **Component / Module:** `crates/cli/src/self_manage.rs`, `scripts/install.sh`, release workflow
- **Suggested patterns:** Publish `scripts/install.sh.sha256` alongside the release; fetch the checksum from the release assets (or embed an expected hash derived from the tag) and verify before `sh`. Gate `GITHUB_LATEST_TAG`/`GITHUB_API_BASE` behind `cfg(test)` or a `KANBAN_ALLOW_UNSAFE_OVERRIDE=1` opt-in.
- **Data model hints:** The checksum source must not be the same raw file fetch (same compromise would defeat it); prefer a release asset or a signed manifest.
- **Testing approach:** Unit-test checksum verification with a known-good and a tampered script; integration-test the pinned-URL construction without network.
- **Migration / backward compatibility:** Existing `kanban upgrade` continues to work against releases that publish the checksum asset; older releases without it fail with a clear "no checksum available" error.

### Estimation Rules

`story_points` is `8` (complexity: high) — needs an ADR, a checksum source, and
flow changes plus tests.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Install script URL is pinned to the resolved release tag
- [ ] SHA-256 is verified before `sh` execution; mismatch aborts
- [ ] `GITHUB_LATEST_TAG`/`GITHUB_API_BASE` overrides are gated or opt-in
- [ ] ADR records the upgrade trust model
- [ ] Tests cover checksum match, mismatch, and pinned-URL construction
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status       | Notes                                       |
| --------------------------------------- | ------- | ------------ | ------------------------------------------- |
| EP-002: GitHub release distribution     | Epic    | In Progress  | Provides the release tag and checksum asset |
| US-007: GitHub Actions release artifacts| Story   | In Progress  | Publishes checksums alongside releases      |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Decide checksum source (release asset vs embedded) and write ADR
- Pin install script URL to `v{version}/scripts/install.sh`
- Add SHA-256 verification before `sh` execution
- Gate unsafe env overrides
- Add verification and mismatch tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should the checksum be embedded in the binary or fetched?        | Tooling lead | 2026-07-04 | Yes — fetched from the same release tag's assets as `install.sh.sha256`. Keeps the trust root decoupled from the binary build and avoids rebuilding for every release. Older releases without the checksum asset fail with a clear "no checksum available" error. |
| 2   | Should `main` fetch be hard-refused, or only warned?             | Tooling lead | 2026-07-04 | Yes — hard-refused. The install script URL must be pinned to a resolved release tag (`v{version}/scripts/install.sh`); resolving to `main` is an error outside `cfg(test)`. |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
