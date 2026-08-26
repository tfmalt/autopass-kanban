# ADR-003: User default backlog repository

## Status

Accepted, 2026-08-26.

## Context

`kanban` can be installed globally while a developer's backlog lives in a
separate Git repository from the code repositories where commands are run. The
CLI previously interpreted every omitted repository argument as the current
directory, so it could not address a separate backlog without repeating its
path.

## Decision

Store an optional canonical `default_repo_root` in the user-level JSON file
`${KANBAN_CONFIG_HOME:-${XDG_CONFIG_HOME:-~/.config}/kanban}/config.json`.

Normal repository commands select a root in this order:

1. Explicit positional root.
2. `KANBAN_REPO_ROOT`.
3. The current Git worktree when its root contains `.kanban/settings.json`.
4. The user default.
5. Current directory if no other source is available.

The configured root must be a Git repository containing
`.kanban/settings.json`. Invalid configured roots fail with guidance and never
fall back to the current directory. `init` does not consume the user default,
so it remains safe to initialize the current repository. The setting is managed
through `kanban config global set-root|show|clear-root`.

## Consequences

The repository's `.kanban/settings.json` continues to be the authoritative
configuration for all backlog content and paths. The user file stores only a
machine-local routing preference and must not contain backlog state.

An omitted positional root must be distinguishable from an explicitly supplied
`.` so explicit invocation can remain authoritative. The CLI therefore uses an
internal marker for omitted roots before root resolution.

## Alternatives Considered

- Store the setting in the current repository's `.kanban/settings.json`:
  rejected because the repository must be selected before that file can be read.
- Always prefer the global default: rejected because it would make local
  backlog repositories surprising and unsafe to operate.
- Silently fall back when a configured path is stale: rejected because it can
  direct mutations at an unintended repository.
