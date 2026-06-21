---
id: US-005
type: user-story
status: done
epic: EP-001
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-06-21T22:39:50+0200
work_done: 2026-06-21T22:47:18+0200
created: 2026-06-21T16:48:56+0200
updated: 2026-06-21T22:47:18+0200
---

# User Story: Remote `curl | sh` installer pinned to a git ref

---

## Story Statement

**As a** developer on a clean machine with no local clone of the repository,
**I want** to install `kanban` and its agent skills by piping a single
`curl`-fetched script to `sh`, with explicit version selection and a documented
escape hatch,
**so that** I can get a working `kanban` on any macOS or Linux machine without
cloning, without a Rust toolchain, and without `sudo`, while still being able
to pin to a specific release for reproducibility.

---

## Background

US-001 through US-004 cover the local install, upgrade, and uninstall flows.
This story makes the same `scripts/install.sh` reachable from a remote machine
via the canonical `curl -fsSL <url> | sh` pattern used by other developer
tooling (rustup, Homebrew, Deno, Bun, etc.).

The script itself is already in the repository at `scripts/install.sh`; this
story adds:

1. A small remote entrypoint (`scripts/install.remote.sh` or a `?remote=1`
   variant of `install.sh`) that detects it was invoked with no `--binary`
   argument and no local source, and fetches the matching release tarball from
   the git repository's release artifacts.
2. Documented `curl | sh` invocations in the README that pin to a tag, a
   commit, or `main` (with a warning about the latter).
3. Non-interactive defaults so `curl | sh` does not hang when stdin is not a
   TTY, while still allowing `--yes` to skip prompts in CI.

This story does not cover publishing the release artifacts themselves (US-006)
or checksum verification (US-006); it consumes them. The two stories should
land together: shipping a `curl | sh` flow without checksum verification would
be irresponsible.

---

## Acceptance Criteria

**Scenario 1: Default remote install from the latest release tag**

```gherkin
Given the user is on a clean macOS machine with `curl` and `sh` available
  and the repository's latest release tag is `v26.7.0101`
When the user runs `curl -fsSL https://<repo>/install.sh | sh`
Then the fetched script:
  - resolves the latest release tag from the repository's release metadata
  - downloads `kanban-26.7.0101-<target>.tar.gz` and its matching checksums file (US-006)
  - verifies the SHA-256 of the tarball against the checksums file
  - extracts the tarball to a temp dir
  - invokes the same install flow as US-001 with `--binary <temp>/kanban`
  - installs agent skills from the tarball (US-002) since stdin is not a TTY and `--yes` is implied
  and a new shell runs `kanban --version` and reports `26.7.0101`
```

**Scenario 2: Pin to a specific version**

```gherkin
Given the user wants to install a specific release
When the user runs `curl -fsSL https://<repo>/install.sh | sh -s -- --version v26.6.2107`
Then the fetched script:
  - downloads `kanban-26.6.2107-<target>.tar.gz` and its checksums file
  - verifies the checksum
  - installs that exact version
  and `kanban --version` reports `26.6.2107`
```

**Scenario 3: Pin to `main` prints a warning**

```gherkin
Given the user runs `curl -fsSL https://<repo>/install.sh | sh -s -- --channel main`
When the fetched script runs
Then it:
  - downloads the artifact attached to the latest successful `main` CI run
  - prints a warning to stderr that installing from `main` is not reproducible
    and recommends pinning to a tag for production use
  - proceeds with the install only after the user passes `--yes` (or `--force`)
```

**Scenario 4: Non-interactive defaults in CI**

```gherkin
Given the user runs `curl -fsSL <url> | sh` in a CI runner where stdin is not a TTY
When the fetched script needs to make a decision (skills dir, downgrade, etc.)
Then it:
  - uses the discovered default for each decision without prompting
  - does not block waiting for input
  - prints a single-line summary of each default it took
  - exits non-zero if any decision cannot be made safely with a default
```

**Scenario 5: Interactive install in a local terminal**

```gherkin
Given the user runs `curl -fsSL <url> | sh` in a real terminal with stdin on a TTY
When the fetched script needs to decide the skills dir
Then it prompts the user as in US-002 (the `/dev/tty` prompt flow)
  and waits for the user's response
```

**Scenario 6: Checksum mismatch aborts before any filesystem write**

```gherkin
Given the downloaded tarball's SHA-256 does not match the checksums file
When the fetched script runs
Then it:
  - prints "Checksum mismatch for <tarball>. Expected <a>, got <b>. Aborting."
  - removes the temp download directory
  - does not invoke the install flow
  - does not write anything to the user's home directory
  - exits with status 1
```

**Scenario 7: Target detection picks the right artifact**

```gherkin
Given the user is on Apple Silicon macOS
When the fetched script runs target detection
Then it selects the `aarch64-apple-darwin` artifact
  and on Intel macOS selects `x86_64-apple-darwin`
  and on x86_64 Linux glibc selects `x86_64-unknown-linux-gnu`
  and on x86_64 Linux musl (Alpine) selects `x86_64-unknown-linux-musl`
  and on aarch64 Linux selects `aarch64-unknown-linux-gnu` (or musl when detected)
```

**Scenario 8: Unsupported target errors clearly**

```gherkin
Given the user is on a target for which no release artifact exists
  (for example `armv7-unknown-linux-musleabihf`)
When the fetched script runs target detection
Then it:
  - prints "No release artifact for target <triple>. Open an issue or build from source: <docs link>"
  - exits with status 1
  - does not attempt to fall back to a different target's binary
```

**Scenario 9: Offline re-run with cached tarball**

```gherkin
Given the user has previously downloaded `kanban-<version>-<target>.tar.gz` to `~/.cache/kanban/`
  and the network is unreachable
When the user runs `curl -fsSL <url> | sh -s -- --version <version> --offline`
Then the fetched script:
  - finds the tarball in the cache
  - verifies its checksum against the cached checksums file
  - installs from the cache without contacting the network
  - prints a notice that the install used the cached artifact
```

**Scenario 10: Escaping the pipe with `sh -s --` flags is documented**

```gherkin
Given the user wants to pass flags to a `curl | sh` invocation
When the user reads the README install section
Then the README documents the `sh -s -- <flags>` pattern with at least three examples:
  - pinning to a version
  - passing `--prefix`
  - passing `--dry-run`
  and notes that flags cannot be passed to `curl` directly because `curl`'s stdout is the script
```

---

## Non-Functional Requirements

| Area                | Requirement                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------- |
| **Portability**     | Target detection uses `uname -s` and `uname -m` only; no dependency on `rustc` or `cargo` |
| **Security**        | Always verifies checksums before extracting; never executes anything from inside the tarball except the `kanban` binary; refuses to use a checksums file that does not list the selected artifact |
| **Traceability**    | Logs the resolved tag/commit, the artifact URL, the expected and actual checksum, and the temp directory used |
| **Auditability**    | After install, `manifest.txt` records the artifact URL and checksum as the `source` field |
| **Performance**     | Total install (download + verify + extract + install) completes in under 15 seconds on a fast connection for a ~10 MB artifact |
| **Backward compatibility** | Honors the same flags as `scripts/install.sh` so the local and remote flows are one script |

---

## Technical Notes

- **Requirement refs:** `EP-001#acceptance-criteria` (curl|sh, version pinning, escape hatch)
- **Component / Module:** `scripts/install.sh` remote-download branch; `scripts/install.remote.sh` thin wrapper if a separate entrypoint is cleaner. README install section.
- **Key integration points:**
  - Consumes release artifacts published by US-006: `kanban-<version>-<target>.tar.gz` and `kanban-<version>-checksums.txt`.
  - Reuses the install flow from US-001 and the skill install flow from US-002.
  - Reuses the prompt helpers from US-002 with TTY-detection gating.
- **Suggested patterns:**
  - A `detect_target` function mapping `(uname -s, uname -m, libc flavour)` to a Rust target triple.
  - A `resolve_version` function that takes `--version <tag>` or `--channel main` and returns a concrete download URL.
  - A `fetch_and_verify` function that downloads tarball + checksums to `~/.cache/kanban/`, verifies SHA-256, and returns the extracted dir.
  - TTY detection via `if [ -t 0 ]` to switch between interactive prompts and non-interactive defaults.
- **Data model hints:** The manifest's `source` field records the artifact URL and checksum joined by `;` for remote installs (e.g. `https://<repo>/releases/download/v26.7.0101/kanban-26.7.0101-aarch64-apple-darwin.tar.gz;sha256=...`).
- **Testing approach:**
  - Unit-test `detect_target` against synthetic `uname` outputs.
  - Integration-test the full `curl | sh` flow against a local HTTP fixture serving a prebuilt tarball and checksums file.
  - Integration-test the checksum-mismatch and unsupported-target failure modes.
  - Integration-test the non-interactive defaults by piping `</dev/null`.
- **Migration / backward compatibility:** None — first introduction of the remote flow.

### Estimation Rules

`story_points` is `5` (M): target detection, version resolution, TTY-aware prompting, and the checksum-mismatch failure path each add surface; the actual install flow is reused from US-001/US-002.

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set when this story moves to `in-progress`.

---

## Definition of Done

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] `curl -fsSL <repo>/install.sh | sh` works on a clean macOS and Linux machine
- [ ] Checksum mismatch aborts before any filesystem write
- [ ] Non-interactive install works in CI (no TTY)
- [ ] Code reviewed and approved via pull request (minimum 1 reviewer)
- [ ] Integration tests in `tests/install/` cover all ten scenarios using a local HTTP fixture
- [ ] README documents the `curl | sh` command with `--version`, `--prefix`, and `--dry-run` examples and the `sh -s --` pattern
- [ ] Workspace version bumped in `Cargo.toml` per the SemVer scheme in `AGENTS.md`
- [ ] `cargo fmt --all --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass

---

## Dependencies

| Dependency                                | Type           | Status    | Notes                                                              |
| ----------------------------------------- | -------------- | --------- | ------------------------------------------------------------------ |
| US-001: Local binary install              | Story          | Draft     | The remote flow invokes the same install helpers as the local flow |
| US-002: Agent skills install              | Story          | Draft     | The remote flow installs skills from the tarball, not from a local clone |
| US-006: Checksum-verified release artifacts | Story        | Draft     | Must land together; the remote flow cannot ship without verified artifacts |
| Release pipeline publishing artifacts     | Infrastructure | Pending   | CI must publish per-target tarballs and a checksums file on tag    |

---

## Sprint Task Log Guidance

Expected tasks once activated into a sprint:

- Implement `detect_target` from `uname -s` / `uname -m` / libc detection
- Implement `resolve_version` for `--version <tag>` and `--channel main`
- Implement `fetch_and_verify` with `~/.cache/kanban/` cache and `--offline`
- Wire remote-download branch into `scripts/install.sh` behind the "no `--binary`" condition
- Implement TTY-aware defaults reusing US-002's prompt helpers
- Add integration tests with a local HTTP fixture and a prebuilt tarball
- Document the `curl | sh` command and `sh -s --` pattern in README and HOWTO.md

---

## Notes and Open Questions

| #   | Question / Assumption                                                                    | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------------------------------- | ------------ | ---------- | -------- |
| 1   | Does the remote script live at `scripts/install.sh` (one file) or `scripts/install.remote.sh` (two)? | Tooling lead | 2026-06-27 | No       |
| 2   | Should `main` channel be supported at all, or only release tags?                         | Tooling lead | 2026-06-27 | No       |
| 3   | Should the cache live at `~/.cache/kanban/` (XDG) or `~/.local/cache/kanban/`?           | Tooling lead | 2026-06-27 | No       |
| 4   | Do we mirror artifacts to a CDN, or serve from the git host directly?                     | Tooling lead | 2026-07-04 | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
