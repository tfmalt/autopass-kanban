---
id: EP-002
type: epic
status: in-progress
phase: 1
owner: Thomas Malt / Tooling Lead
milestone: MP1
created: 2026-06-22T00:00:00+0200
updated: 2026-06-22T10:41:16+0200
---

# Epic: GitHub-hosted cross-platform release distribution

---

---

## Business Context

The repository now lives on GitHub, so installability should rely on GitHub
Releases and GitHub Actions rather than an unspecified CI host. Users need a
stable release URL that serves the right binary for their operating system and
processor without cloning the repository or building Rust locally.

---

## Business Value

- **Primary benefit:** Users can install or upgrade `kanban` from GitHub with a
  single curl-to-bash command that resolves the correct release artifact.
- **Secondary benefit:** Maintainers get a repeatable release pipeline for macOS,
  Linux, and Windows artifacts across x86_64 and ARM64 CPUs.
- **Risk if not done:** The installer points at stale release locations and users
  cannot reliably obtain platform-specific binaries from GitHub.

---

## Users and Stakeholders

| Role                 | Involvement                                                       |
| -------------------- | ----------------------------------------------------------------- |
| New user / developer | Installs `kanban` from GitHub without Rust or a local clone        |
| Existing contributor | Pins a release version for reproducible upgrades                  |
| Tooling lead         | Publishes and audits release assets                               |
| GitHub Actions       | Builds and attaches installable release archives and checksums    |

---

## Scope

### In Scope

- GitHub Actions workflow that builds release archives on tag push.
- Versioned archives for macOS, Linux, and Windows x86_64/ARM64 targets where
  the Rust toolchain and GitHub-hosted runners support the target.
- A checksums file uploaded beside the archives.
- Installer defaults that resolve the latest GitHub release when no explicit
  `--version` is supplied.
- README documentation for curl-to-bash install, version pinning, and artifact
  names.

### Out of Scope

- Native Windows package managers (`winget`, `scoop`, MSI).
- Code signing, notarization, SBOMs, or Sigstore attestations.
- Replacing the POSIX installer with a PowerShell installer.

---

## Acceptance Criteria

- [ ] Pushing tag `v<workspace-version>` creates a GitHub Release containing
      installable `kanban-<version>-<target>.tar.gz` archives and
      `kanban-<version>-checksums.txt`.
- [ ] Release archives include the `kanban` binary (`kanban.exe` on Windows),
      both agent skills, and a `VERSION` file matching the tag.
- [ ] The installer can be run as
      `curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash`
      and installs the latest release for the detected macOS or Linux target.
- [ ] Passing `--version v<version>` to the curl-to-bash command installs that
      exact GitHub release and verifies its checksum before extraction.
- [ ] Unsupported targets fail with a clear error and do not fall back to a
      different target binary.

---

## Non-Functional Requirements

| Area            | Requirement                                                                 |
| --------------- | --------------------------------------------------------------------------- |
| **Security**    | Installer verifies SHA-256 checksums before extracting release archives      |
| **Portability** | Release archives use portable `tar.gz` layout across OS-specific binaries    |
| **Traceability**| Release notes and assets identify the source tag and commit SHA              |
| **Auditability**| Checksums can be reproduced locally with `scripts/release/checksums.sh`      |

---

## Architecture Considerations

- **Relevant architecture principles:** Markdown and Git remain the source of
  truth; GitHub Releases are distribution artifacts, not runtime state.
- **Key patterns in play:** Tag-driven CI release workflow, versioned artifact
  naming, checksum verification before install.
- **ADR references:** None yet.
- **Known risks or constraints:** GitHub-hosted runner availability and cross
  compilation support may limit some ARM targets; unsupported targets must fail
  explicitly rather than installing a nearby binary.

---

## Dependencies

| Dependency        | Type     | Status    | Notes                                      |
| ----------------- | -------- | --------- | ------------------------------------------ |
| GitHub repository | Platform | Available | `tfmalt/autopass-kanban` is the origin repo |
| EP-001            | Epic     | Done      | Existing installer and release contracts    |

---

## Child User Stories

| Story ID | Title                                                        | Status | Points |
| -------- | ------------------------------------------------------------ | ------ | ------ |
| US-007   | GitHub Actions release artifacts and latest curl installer   | In Progress | 5      |

---

## Definition of Done (Epic Level)

- [ ] All child User Stories are complete and accepted
- [ ] GitHub release assets are published from a tag in the expected layout
- [ ] README documents the GitHub curl install and version pinning commands
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace
      --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Notes and Open Questions

| #   | Question / Assumption                                           | Owner        | Due        | Resolved |
| --- | --------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Should GitHub release immutability be enforced by repository settings? | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic Epic template derived from the kanban tooling conventions_
