---
id: US-006
type: user-story
status: draft
epic: EP-001
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started:
work_done:
created: 2026-06-21T16:48:56+0200
updated: 2026-06-21T16:48:56+0200
---

# User Story: Versioned, checksum-verified release artifacts consumed by the installer

---

## Story Statement

**As a** tooling lead publishing `kanban` releases,
**I want** the release pipeline to publish one tarball per supported target
plus a single checksums file listing every tarball's SHA-256, tagged with the
workspace version,
**so that** the local and remote installers (US-001, US-005) can verify
integrity before writing anything to a user's home directory and so that
supply-chain attacks and silent regressions are detectable.

---

## Background

The installer flows in US-001 and US-005 both need a stable, predictable
release artifact layout to download (or to point a local `--binary` at when
testing a release locally). Without published artifacts, US-005 cannot ship at
all, and US-001 has no canonical "release binary" to install. This story
defines the artifact naming, the checksums file format, the publish trigger,
and the verification contract the installer relies on.

The scope is deliberately narrow: produce and publish the artifacts and the
checksums file. Deep signing (PGP / Sigstore), a CDN mirror, and a
transparency log are out of scope and belong to a follow-up Epic. SHA-256
against a checksums file committed alongside the release is the bar this Epic
sets.

---

## Acceptance Criteria

**Scenario 1: Tagged release publishes one tarball per supported target**

```gherkin
Given a maintainer pushes git tag `v26.7.0101`
  and CI runs on tag push
When the release workflow runs
Then it builds `kanban` for each supported target triple:
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
  - `x86_64-unknown-linux-gnu`
  - `x86_64-unknown-linux-musl`
  - `aarch64-unknown-linux-gnu`
  and uploads `kanban-26.7.0101-<triple>.tar.gz` as a release asset on the tag
  and each tarball contains a top-level `kanban` binary plus a `skills/` directory
    with both kanban skills and a `VERSION` file
```

**Scenario 2: Tarball layout is stable and documented**

```gherkin
Given the user inspects any `kanban-<version>-<triple>.tar.gz`
When the tarball is extracted
Then it contains exactly:
  - `kanban` (the prebuilt binary, executable bit set)
  - `skills/kanban-backlog-maintainer/SKILL.md`
  - `skills/kanban-backlog-maintainer/plugin.json`
  - `skills/kanban-developer/SKILL.md`
  - `skills/kanban-developer/plugin.json`
  - `VERSION` (a single line with the workspace version, e.g. `26.7.0101`)
  and no other files
  and the layout is identical across all target tarballs except for the binary itself
```

**Scenario 3: One checksums file lists every tarball**

```gherkin
Given the release workflow has uploaded all tarballs for `v26.7.0101`
When the workflow finishes
Then it also uploads `kanban-26.7.0101-checksums.txt` as a release asset
  and the file contains one line per tarball in the format:
    `<sha256>  kanban-26.7.0101-<triple>.tar.gz`
  and the file is sorted by filename
  and the file uses two-space separation (the standard `sha256sum` format)
```

**Scenario 4: Installer verifies a tarball against the checksums file**

```gherkin
Given US-005 has downloaded `kanban-26.7.0101-x86_64-apple-darwin.tar.gz`
  and `kanban-26.7.0101-checksums.txt`
When the installer runs checksum verification
Then it:
  - computes the SHA-256 of the downloaded tarball
  - greps the checksums file for a line matching the tarball's filename
  - aborts with exit status 1 if the line is missing
  - aborts with exit status 1 if the computed hash does not match the listed hash
  - proceeds with extraction only on an exact match
```

**Scenario 5: Checksums file is reproducible from the tarballs**

```gherkin
Given a maintainer has the published tarballs for `v26.7.0101` on disk
When the maintainer runs `scripts/release/checksums.sh v26.7.0101`
Then the script prints a checksums file whose contents are byte-identical to
  `kanban-26.7.0101-checksums.txt` published by CI
  (so the published file can be audited against a manual rebuild)
```

**Scenario 6: Version embedded in the binary matches the tag**

```gherkin
Given the release workflow built the binary for tag `v26.7.0101`
When the installer extracts the tarball and runs `./kanban --version`
Then the printed version is `26.7.0101`
  and matches the `VERSION` file in the tarball
  and matches the tag (minus the leading `v`)
  (so a misconfigured release that ships the wrong version is detectable before install)
```

**Scenario 7: Release artifact names match the contract US-005 expects**

```gherkin
Given US-005's `resolve_version` builds a download URL
  of the form `<repo>/releases/download/v<version>/kanban-<version>-<triple>.tar.gz`
When the release workflow publishes the artifacts
Then the artifact filenames and the release tag naming match that contract exactly
  (documented in `SPEC-installability.md` and tested by a CI job that dry-runs
   the remote installer against the just-published release)
```

**Scenario 8: Failed build for one target does not block the others**

```gherkin
Given the `aarch64-unknown-linux-gnu` build fails in the release workflow
When the workflow finishes
Then the tarballs for the other four targets are still published
  and the checksums file lists only the successfully built tarballs
  and the workflow reports the failed target as a non-fatal job failure
  (so a single broken cross-compile target does not block a release)
```

**Scenario 9: Release artifacts are immutable**

```gherkin
Given the release for `v26.7.0101` has been published
When a maintainer attempts to overwrite one of the tarballs or the checksums file
Then the git host rejects the upload because the release is marked immutable
  (or, on hosts that allow edits, the release workflow records the original
   checksums in a separate `kanban-<version>-checksums.txt.asc`-style appendix
   that is never overwritten — to be resolved in Open Question 2)
```

---

## Non-Functional Requirements

| Area                | Requirement                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------- |
| **Performance**     | The full release workflow (all five targets) completes in under 20 minutes on CI         |
| **Portability**     | Tarballs use `tar -czf` with no platform-specific flags; extract cleanly on macOS, Linux, and Alpine |
| **Security**        | Checksums file is published as a release asset alongside the tarballs, not in a separate mutable branch; SHA-256 only (no MD5/SHA-1) |
| **Traceability**    | Each release asset lists the CI workflow run URL and commit SHA in its release notes     |
| **Auditability**    | `scripts/release/checksums.sh` reproduces the published checksums file from local tarballs |
| **Reproducibility** | Builds are reproducible given the same commit and Rust toolchain; CI pins the Rust toolchain version |

---

## Technical Notes

- **Requirement refs:** `EP-001#acceptance-criteria` (checksum verification), US-005 download contract
- **Component / Module:**
  - `.github/workflows/release.yml` (or equivalent) — the release workflow.
  - `scripts/release/checksums.sh` — a small POSIX script that builds the checksums file from a set of tarballs.
  - `SPEC-installability.md` — the artifact naming contract (authoritative).
- **Key integration points:**
  - Consumes `cargo build -p kanban-cli --release --target <triple>` for each target.
  - Consumes `skills/kanban-backlog-maintainer/` and `skills/kanban-developer/` for the in-tarball `skills/` directory.
  - Produces artifacts consumed by US-001 (via `--binary <extracted>/kanban`) and US-005 (via the remote download flow).
- **Suggested patterns:**
  - A per-target build matrix job that builds, packs the tarball, and uploads it as a release asset.
  - A post-build job that depends on all build jobs, downloads the just-uploaded tarballs, computes the checksums file, and uploads it.
  - The `VERSION` file is written from the workspace version in `Cargo.toml` so the binary, the `VERSION` file, and the tag all agree.
- **Data model hints:** The checksums file format is the standard `sha256sum` format so it can be verified with `sha256sum -c checksums.txt` on Linux and `shasum -a 256 -c checksums.txt` on macOS.
- **Testing approach:**
  - Test `scripts/release/checksums.sh` with a fixture set of tarballs and assert the output format.
  - Add a CI job (post-release) that dry-runs `scripts/install.sh --version <just-published-tag> --dry-run` against the just-published release to confirm the installer can resolve and verify it.
  - Assert that `kanban --version` in the extracted tarball matches the tag.
- **Migration / backward compatibility:** This is the first release pipeline; there is no prior artifact format to migrate.

### Estimation Rules

`story_points` is `5` (M): five-target build matrix, checksum generation, immutability considerations, and the post-release verification job each add work; none individually is large.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set when this story moves to `in-progress`.

---

## Definition of Done

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] Tagging `v<version>` publishes all five tarballs plus the checksums file
- [ ] `scripts/release/checksums.sh` reproduces the published checksums file byte-for-byte
- [ ] US-005's `curl | sh` flow can install from the just-published release in a CI job
- [ ] Code reviewed and approved via pull request (minimum 1 reviewer)
- [ ] `SPEC-installability.md` documents the artifact naming contract
- [ ] README documents how to verify a download manually with `sha256sum -c`
- [ ] Workspace version bumped in `Cargo.toml` per the SemVer scheme in `AGENTS.md`
- [ ] `cargo fmt --all --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Dependencies

| Dependency                              | Type           | Status    | Notes                                                              |
| --------------------------------------- | -------------- | --------- | ------------------------------------------------------------------ |
| CI runner with Rust toolchain           | Infrastructure | Available | Already used for the existing build/test pipeline                  |
| Cross-compile targets in `rustup`       | Infrastructure | Available | `rustup target add` in the release workflow                        |
| US-005: Remote `curl \| sh` installer   | Story          | Draft     | Must land together; the installer contract and the artifact contract are co-defined |
| `SPEC-installability.md` artifact section | Document     | Draft     | Records the authoritative naming contract                          |

---

## Sprint Task Log Guidance

Expected tasks once activated into a sprint:

- Add `.github/workflows/release.yml` with a per-target build matrix triggered on tag push
- Pack the tarball with `kanban`, `skills/`, and `VERSION`
- Generate and upload `kanban-<version>-checksums.txt`
- Add `scripts/release/checksums.sh` for local reproducibility
- Add a post-release CI job that dry-runs the remote installer against the just-published release
- Document the artifact naming contract in `SPEC-installability.md`
- Document manual `sha256sum -c` verification in README

---

## Notes and Open Questions

| #   | Question / Assumption                                                                  | Owner        | Due        | Resolved |
| --- | -------------------------------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Do we ship musl-static binaries for `*-linux-gnu` targets too, or only for Alpine?     | Tooling lead | 2026-07-04 | No       |
| 2   | Should the git host's "do not allow edits to releases" setting be enforced in repo config? | Tooling lead | 2026-07-04 | No       |
| 3   | Do we publish an SBOM (CycloneDX or SPDX) alongside the checksums file in this Epic?   | Tooling lead | 2026-07-04 | No       |
| 4   | Where do `aarch64-unknown-linux-musl` and `armv7-*` targets fit, if at all?            | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
