# ADR-002: Upgrade trust root — pinned release tag + checksum asset

- **Status:** Accepted
- **Date:** 2026-06-24
- **Supersedes:** none
- **Related:** US-010, EP-003, EP-002

## Context

Before US-010, `kanban upgrade` fetched
`https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh`
and piped it into `sh` with no checksum or signature. The only trust root was
TLS to GitHub. A compromise of the `main` branch (or the GitHub account) would
yield remote code execution on every upgrading user. The advertised checksum
verification only covered the binary the script later downloads, not the
script itself.

Additionally, `resolve_latest_version` honored `GITHUB_LATEST_TAG` and
`GITHUB_API_BASE` environment variables that could suppress or redirect the
update check, weakening the trust model further.

## Decision

1. **Pinned tag fetch.** The install script is fetched from
   `https://raw.githubusercontent.com/tfmalt/autopass-kanban/<tag>/scripts/install.sh`,
   where `<tag>` is the resolved release tag (`v<version>`). Fetching from
   `main` is hard-refused by `ensure_pinned_tag` outside test configuration.
2. **Checksum from a second channel.** A SHA-256 digest is fetched from the
   same release tag's asset `install.sh.sha256` (`<script_url>.sha256`) and
   verified against the downloaded script bytes before `sh` executes it. The
   checksum asset is a separate object from the script; a single-file
   compromise cannot forge both. Releases without the checksum asset fail with
   a clear "no checksum available" error.
3. **Gated unsafe overrides.** `GITHUB_LATEST_TAG` and `GITHUB_API_BASE` are
   honored only under `cfg(test)` or when an operator explicitly sets
   `KANBAN_ALLOW_UNSAFE_OVERRIDE=1`. Outside those conditions the real GitHub
   API is used. This lets tests run without network while keeping production
   on the trusted path.
4. **Trust implications documented** here and referenced from `AGENTS.md`.

## Consequences

- **Positive:** A compromise of `main` no longer affects upgrading users; only
  a published, checksummed release tag is executed. The JSON `code`/exit
  behavior on checksum mismatch is a clear non-zero abort before execution.
- **Negative:** Older releases that did not publish `install.sh.sha256` cannot
  be upgraded to via this flow and fail with a clear error. Operators behind
  air-gapped mirrors must set `KANBAN_ALLOW_UNSAFE_OVERRIDE=1` and accept the
  trust implications.
- **Future:** Code signing / Sigstore for release artifacts belongs with
  EP-002 and can layer on top of this checksum model.
