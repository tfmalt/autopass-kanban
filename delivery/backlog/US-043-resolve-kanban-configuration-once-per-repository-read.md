---
id: US-043
type: user-story
status: done
epic: EP-004
sprint: S001.rolling-thunder
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 8
priority: 20
work_started: 2026-08-04T10:00:13+0200
work_done: 2026-08-04T10:00:13+0200
created: 2026-08-04T10:00:13+0200
updated: 2026-08-27T10:06:07+0200
activated: 2026-08-27T10:06:07+0200
---

# User Story: Resolve kanban configuration once per repository read

---

## Story Statement

**As a** user of the `kanban` CLI and web server,
**I want** one repository read to resolve the git root and parse
`.kanban/settings.json` exactly once,
**so that** reading a backlog costs one subprocess spawn instead of one per file.

---

## Background

`read_repository` loaded the configuration and then called
`collect_user_story_files` and `read_story_file`, both of which loaded it again —
the latter once per story. Every `load_kanban_config` called `resolve_repo_root`,
which spawned `git -C <path> rev-parse --show-toplevel` and re-read and re-parsed
`.kanban/settings.json`. There was no memoization anywhere in the workspace.

`find_epic` compounded it: a full `read_repository` plus a full epic-file rescan
per call.

Measured on this repository (41 stories, release build): `kanban validate .`
0.56 s wall against 0.14 s user, `kanban doctor .` 0.87 s wall against 0.49 s
user. High wall time against low user time is subprocess spawning, not parsing.

---

## Acceptance Criteria

**Scenario 1: One repository read resolves configuration once**

```gherkin
Given a generated backlog fixture
When `read_repository` is called
Then exactly one `git rev-parse --show-toplevel` subprocess is spawned
And `.kanban/settings.json` is parsed exactly once
And each story file is parsed exactly once
```

**Scenario 2: Parsed output is unchanged**

```gherkin
Given a generated backlog fixture in either feature configuration
When a story is read through the config-aware reader and through the
    convenience reader
Then the relative path, absolute path, frontmatter, frontmatter key order, body,
    markdown, sprint name and task-file resolution are identical
```

**Scenario 3: Path containment is unchanged**

```gherkin
Given a story whose `task_file` resolves outside the backlog root,
    directly or through a symlink
When the story is read
Then the task file is reported as non-existent and is not read
```

**Scenario 4: The CLI gets the same fix**

```gherkin
Given a backlog repository
When `kanban validate .` and `kanban doctor .` run
Then their output is unchanged
And `kanban doctor` completes within 500 ms on a 250-story fixture
```

---

## Non-Functional Requirements

| Area | Requirement |
| ---- | ----------- |
| **Performance** | `kanban doctor` <= 500 ms on the 250-story fixture |
| **Backward compatibility** | No public `crates/core` API removed |
| **Data integrity** | Path containment, symlink handling and story ordering byte-identical |

---

## Technical Notes

- **Requirement refs:** `EP-004#acceptance-criteria`
- **Component / Module:** `crates/core/src/{config,repository,epic,sprint,story,validate,doctor,phase}.rs`
- **Chosen approach:** thread `&KanbanConfig` through config-aware variants —
  `collect_user_story_files_with_config`, `collect_epic_files_with_config`,
  `read_story_file_with_config`, `read_epic_file_with_config`,
  `read_repository_with_config`, `summarize_sprints_from_repository`,
  `validate_parsed_repository`, `find_epic_in_repository`, `read_epic_sources`,
  `select_epic_source`, `epic_details_from_source`,
  `story_overviews_from_repository`.
- **Rejected approach:** memoize `resolve_repo_root`/`load_kanban_config` in a
  process-local map keyed by canonical path. Far smaller, and not forbidden by
  the "no persisted cache" constraint, but a library crate that silently caches
  filesystem state is a latent bug for `config set` and for any test that mutates
  `settings.json` in-process.
- **Deliberate preservation:** `read_story_file(path, repo_root)` still computes
  relative paths against the **caller's** `repo_root`, not `config.repo_root`.
  These differ when a caller passes `"."`, and CLI call sites depend on the
  existing behavior.
- **Incidental fix:** `find_epic_with_source` now resolves child-story epic
  titles against the canonical repository root rather than the caller-supplied
  path, so it no longer depends on the process working directory.
- **Constant-factor follow-on:** shared regular expressions moved to
  `crates/core/src/regexes.rs` as `LazyLock` statics. They were previously
  recompiled per story and per task file, which dominated the residual cost of
  `validate` and `doctor` once the configuration blowup was gone.

### Estimation Rules

`story_points` is `8` (complexity: high).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [x] `read_repository` performs exactly one root resolution and one settings parse
- [x] No configuration load inside any per-file loop in `crates/core`
- [x] Golden-output equivalence test on both fixture configurations
- [x] Symlink-escape and unsafe-`task_file` containment tests unchanged
- [x] `kanban validate .` and `kanban doctor .` output unchanged
- [x] Full verification suite passes

---

## Dependencies

| Dependency | Type | Status | Notes |
| ---------- | ---- | ------ | ----- |
| US-042 | Story | Done | Supplies the fixtures and counters this story asserts against |
| US-008 | Story | Done | `ensure_path_inside` containment must be preserved exactly |

---

## Notes and Open Questions

| #   | Question / Assumption | Owner | Due | Resolved |
| --- | --------------------- | ----- | --- | -------- |
| 1 | Should configuration be memoized instead of threaded? Rejected; rationale recorded above and in `IMPROVEMENT_PLAN.md` §WP-02 | Tooling lead | 2026-08-04 | Yes |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
