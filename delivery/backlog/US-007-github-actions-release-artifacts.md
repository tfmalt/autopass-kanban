---
id: US-007
type: user-story
status: in-progress
epic: EP-002
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-22T10:39:36+0200
work_done: 
created: 2026-06-22T00:00:00+0200
updated: 2026-06-22T10:39:36+0200
---

# User Story: GitHub Actions release artifacts and latest curl installer

---

## Story Statement

**As a** developer installing `kanban` from GitHub,
**I want** GitHub Actions to publish installable release artifacts for macOS,
Linux, and Windows CPU targets and the curl-to-bash installer to choose the
right released version,
**so that** I can install or upgrade without cloning the repository, building
Rust locally, or manually selecting an incompatible binary.

---

## Background

US-005 and US-006 introduced a remote installer and versioned release artifact
contract, but they predate the switch to GitHub and only describe macOS/Linux
artifacts. This story makes GitHub the concrete release host, adds a GitHub
Actions workflow, broadens the artifact matrix to include Windows binaries, and
updates `scripts/install.sh` so a no-argument curl-to-bash invocation installs

---

## Acceptance Criteria

**Scenario 1: Tag push publishes GitHub release assets**

```gherkin
Given a maintainer pushes tag `v26.6.2201`
When GitHub Actions runs the release workflow
Then it builds and uploads `kanban-26.6.2201-<target>.tar.gz` assets for the supported macOS, Linux, and Windows targets
And uploads `kanban-26.6.2201-checksums.txt` listing every uploaded archive
```

**Scenario 2: Archives contain installable payloads**

```gherkin
Given any release archive is extracted
When the user inspects its top-level files
Then it contains the platform binary, `skills/kanban-backlog-maintainer/`, `skills/kanban-developer/`, and `VERSION`
And the `VERSION` file matches the release tag without the leading `v`
```

**Scenario 3: Curl-to-bash defaults to the latest GitHub release**

```gherkin
Given the latest GitHub release is `v26.6.2201`
When the user runs `curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash`
Then the installer resolves `v26.6.2201`
And downloads the archive whose target triple matches the user's OS and CPU
And verifies the archive against `kanban-26.6.2201-checksums.txt` before extraction
```

**Scenario 4: Version pinning installs the exact requested release**

```gherkin
Given release `v26.6.2115` exists on GitHub
When the user runs `curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash -s -- --version v26.6.2115`
Then the installer downloads only assets from `v26.6.2115`
And `kanban --version` reports `26.6.2115` after install
```

**Scenario 5: Unsupported targets fail safely**

```gherkin
Given the installer detects an OS/CPU pair with no published archive
When remote install starts
Then it exits non-zero with a clear unsupported-target message
And it does not download or install a fallback binary for another target
```

---

## Non-Functional Requirements

| Area            | Requirement                                                               |
| --------------- | ------------------------------------------------------------------------- |
| **Security**    | Checksum verification happens before extraction or installation            |
| **Portability** | Installer target detection uses OS and CPU information available in shell  |
| **Traceability**| Workflow archives include the version and release metadata in asset names  |
| **Auditability**| `scripts/release/checksums.sh` can reproduce the published checksums file  |

---

## Technical Notes

- **Requirement refs:** `EP-002#acceptance-criteria`, `US-005`, `US-006`
- **Component / Module:** `.github/workflows/release.yml`, `scripts/install.sh`,
  `scripts/release/checksums.sh`, `README.md`
- **Key integration points:** GitHub Releases, Rust target matrix, installer
  artifact naming contract.
- **Suggested patterns:** Use a tag-push workflow with a matrix build job, upload
  archives as workflow artifacts, then publish them plus sorted checksums to the
  GitHub Release in one final job.
- **Data model hints:** Archive names remain
  `kanban-<version>-<target-triple>.tar.gz`; Windows archives contain
  `kanban.exe` while Unix archives contain `kanban`.
- **Testing approach:** Run installer integration tests, dry-run latest-release
  resolution via an environment-provided latest tag, validate the release
  workflow YAML syntactically by review, and rely on GitHub Actions for actual
  cross-runner execution.
- **Migration / backward compatibility:** Existing `--version` installs continue
  to work; no-argument remote install now means latest GitHub release instead of
  a usage error.

### Estimation Rules

`story_points` is `5` because the change touches CI, release artifact layout,
installer target/version resolution, and documentation.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set when this story
  moves to `in-progress`.

---

## Definition of Done

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] `.github/workflows/release.yml` publishes release archives and checksums on tag push
- [ ] `scripts/install.sh` supports no-argument curl-to-bash latest release installs
- [ ] README documents latest and pinned GitHub install commands
- [ ] Workspace version bumped in `Cargo.toml` per `AGENTS.md`
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Dependencies

| Dependency                              | Type           | Status    | Notes                                      |
| --------------------------------------- | -------------- | --------- | ------------------------------------------ |
| GitHub Actions                          | Infrastructure | Available | Runs build and release workflow             |
| US-005: Remote `curl \| sh` installer   | Story          | Done      | Existing remote installer base              |
| US-006: Checksum release artifacts      | Story          | Done      | Existing checksum and naming contract       |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add GitHub Actions release workflow for target archive builds
- Update installer latest-release and target resolution
- Document GitHub curl install commands and artifacts
- Add or update installer tests for latest-release dry-run behavior

---

## Notes and Open Questions

| #   | Question / Assumption                                           | Owner        | Due        | Resolved |
| --- | --------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | x86_64 release assets cover both AMD64 and modern Intel 64-bit CPUs. | Tooling lead | 2026-06-22 | Yes      |
| 2   | Native Windows package installation is out of scope; this story ships Windows binaries as release archives. | Tooling lead | 2026-06-22 | Yes |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
