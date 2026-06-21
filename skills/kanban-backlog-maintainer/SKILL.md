---
name: kanban-backlog-maintainer
description: >
  Maintain and operate a markdown-based kanban backlog and sprint board using the
  `kanban` CLI. Use this skill for creating, editing, validating, planning,
  tracking, or reporting Epics, User Stories, tasks, sprints, and backlog
  quality work. Use `--format json` for read-only commands and `kanban` CLI
  operations first before falling back to manual file edits.
---

# Kanban Backlog Maintainer

Maintain backlog quality and operate the sprint board via the `kanban` CLI. Use
`kanban` commands first for lifecycle, sprint, task, and reporting operations.
Fall back to direct file edits only when a `kanban` command does not yet exist
for the needed operation.

## Prerequisites

- The target repository must be a git repository with `.kanban/paths.json` (run
  `kanban init` from the repo root if `.kanban/` is missing).
- A `delivery/backlog/README.md` or similar project convention document should
  define the project's ID scheme, artifact layout, state model, and templates.

## Required Reading

Before creating or editing backlog artifacts, read in this order:

1. `delivery/backlog/README.md` — authoritative IDs, layout, workflow, and
   quality conventions (adjust the path if `.kanban/paths.json` configures a
   different backlog directory).
2. Parent artifact (Epic before Story).
3. Relevant templates in the project's template directory.
4. Project-specific foundation, architecture, and requirements documents as
   referenced in the backlog README.

For read-only sprint queries, prefer `kanban --format json` first; read files
only when the JSON output is insufficient.

## Core Rules

- Treat Markdown as the source of truth, but prefer `kanban` write commands
  over hand edits because they keep paths, frontmatter, and timestamps
  consistent.
- Run `kanban` from the repo root. Use `--format json` for all read commands
  (`show`, `list`, `current`, `validate`, `doctor`, status queries).
- Read existing files before manual edits. After structural or lifecycle edits,
  run `kanban validate --format json`; run `kanban doctor --format json` when
  lifecycle placement may be affected.
- Never move story/task files or edit `status`, `sprint`, `activated`,
  `work_started`, or `work_done` by hand unless a `kanban` command cannot do it
  and you explicitly flag the fallback.
- Never delete backlog files or tasks unless the user explicitly requests it.
  Completed tasks are evidence for retrofitting and closure.
- Use full local ISO 8601 timestamps with numeric timezone offset for lifecycle
  fields.
- If `.kanban/` is missing, tell the user and suggest `kanban init`.
- Respect the project's language conventions.

## Common Conventions

The kanban tool is project-agnostic, but projects typically follow these
patterns (confirm with `delivery/backlog/README.md`):

| Item | Typical Convention |
|------|--------------------|
| Epic file | `delivery/backlog/phase-N-*/EP-{id}-{slug}.md` |
| Story file | `delivery/backlog/phase-N-*/EP-{id}-*/US-{id}-{slug}.md` |
| Task file | `{US-filename}.tasks.md`, sibling to the story |
| Story states | `draft`, `backlog`/`ready`, `todo`, `in-progress`, `ready-for-qa`, `blocked`, `done`, `dropped` |
| Task states | `todo`, `in-progress`, `blocked`, `done` |
| Phase folders | `phase-N-{name}` under the backlog directory |

Tasks are inline blocks in `.tasks.md`, not one file per task. Task IDs are
`TASK-{STORY-ID}-NNN` in `##` headings.

## Command Map

| Need | Command |
|------|---------|
| Active sprint and progress | `kanban sprint current --format json` |
| List all sprints | `kanban sprint list --format json` |
| Specific sprint | `kanban sprint show <name> --format json` |
| Current/next/all/sprint stories | `kanban story list --current --format json` / `--next` / `--all` / `--sprint <S>` |
| Phase overview | `kanban phase show <phase> --format json` |
| Story details and paths | `kanban story show <id> --format json` |
| Plan story into sprint | `kanban story plan --sprint <S> <id>` |
| Move story status | `kanban story move <id> <status> [-a "Name <email>"]` |
| Assign story | `kanban story update <id> --assignee "Name <email>"` |
| Add task | `kanban task add <id> --title <t> --description <d> [--status <s>] [--tags a,b]` |
| Update task | `kanban task update <id> <task_id> [--status <s>] [...]` |
| Validate consistency | `kanban validate --format json`; `kanban doctor --format json`; `kanban doctor fix` |
| Sprint create/rollover | `kanban sprint create [...]`; `kanban sprint rollover <name>` |
| WBS Excel report | `kanban --format json report wbs \| python3 scripts/wbs_report.py` |

When a requested operation has no command, check `kanban <area> --help`, name
the gap, suggest a concrete future command, then do the smallest safe manual
edit.

## Workflows

### Create Or Update Epic/Story

Use the required reading, then follow the project's template exactly:

1. Confirm the parent Epic for stories; create the Epic first if missing.
2. Assign the next valid ID per the project's ID scheme in the backlog README.
3. Use the project's template (e.g. `TEMPLATE-user-story.md`, `TEMPLATE-epic.md`).
4. Fill mandatory frontmatter: `id`, `title`, `status` (`draft` for new
   stories), `sprint` (`~` until planned), `created`, `updated`, plus
   project-specific traceability fields.
5. Keep acceptance criteria independently testable and traceable to AC anchors
   where the project convention requires.
6. Include explicit scope boundaries, non-functional constraints,
   implementation/testing guidance, Definition of Done, and open questions.
7. Keep parent-child links coherent: story `epic` references and Epic
   `user_stories` list.
8. Save in the correct phase/Epic folder and run validation.

Before saving any story, verify: unique ID per the project's scheme, mandatory
frontmatter filled or explicitly N/A with reason, testable acceptance criteria,
explicit NFRs, concrete DoD, in/out scope, no implementation-blocking open
questions, correct parent Epic link.

### Edit/Validate Existing Artifact

1. Run `kanban story show <id> --format json` when editing a story to confirm
   path and state.
2. Read the confirmed file before editing.
3. Apply precise diffs, not wholesale rewrites, unless replacing a generated
   artifact by request.
4. Run `kanban validate --format json` after structural changes.

### Inspect Backlog Or Sprint

Use `kanban sprint current --format json`, `kanban story list --current --format
json`, `kanban story show <id> --format json`, and `kanban phase show <phase>
--format json`. Report the active sprint, rollup, per-story status and task
counts, blocked work, and a practical next action.

### Activate Story Into Sprint

1. Resolve target sprint with `kanban sprint current --format json` if needed.
2. Run `kanban story plan --sprint <S> <id>`.
3. Confirm with `kanban story show <id> --format json`.
4. Suggest the first 2-5 tasks.

### Manage Tasks

Ask what the user is working on now or next; do not invent a comprehensive task
list upfront. Add small verb-first tasks with short descriptions:

```bash
kanban task add <STORY-ID> --title "<verb-first title>" --description "<short description>" [--status todo|in-progress|blocked|done] [--tags tag1,tag2]
kanban task update <STORY-ID> <TASK-ID> [--status done] [--title ...] [--description ...] [--tags ...]
```

### Move Story Status

Use `kanban story move <id> <status> [-a "Name <email>"]` for every transition.
Use `kanban story update <id> --assignee "Name <email>"` to assign without
moving. Do not recompute sprint progress manually; report `kanban sprint
current` output.

### Done Gate

Before moving to `ready-for-qa` or `done`, prove every acceptance criterion and
linked scenario is resolved:

1. Read `kanban story show <id> --format json`, the story markdown, and the task
   log.
2. List each acceptance criterion as verified, descoped/deferred/N/A with
   reason, or unresolved.
3. If anything is unresolved, stop and ask how to handle each item.
4. Record the decision in the story prose or `## Actual Implementation` before
   moving.
5. Only then run `kanban story move <id> done`. If closing with gaps, record
   exactly which criteria remain open and why.

### Retrofit Story From Tasks

When work is done or mostly done, update the story so it reflects reality:

1. Run `kanban story show <id> --format json`; read the story and `.tasks.md`.
2. Summarize done work, discoveries, scope changes, unfinished work, and test
   evidence.
3. Propose edits before applying them. Preserve original acceptance criteria.
4. If implementation differed from plan, add:

```markdown
## Actual Implementation

> Added during sprint <sprint-name>. Reflects what was actually built.

[Summary of completed work, decisions, scope dropped/deferred, test evidence.]
```

Apply the Done Gate before final status transition.

### Generate WBS Excel Report

Run from repo root:

```bash
pip3 install openpyxl   # only if missing
kanban --format json report wbs | python3 scripts/wbs_report.py
```

The output path defaults to a timestamped file. Use `--output <path>` on the
Python script for an explicit target.

Workbook sheets include WBS, Phase Summary, Sprint Burndown, and Legend & Guide.

If output is stale: missing stories usually lack `epic` frontmatter; blank
`work_started`/`work_done` usually means lifecycle was bypassed; zero velocity
usually means no past sprint delivered points. Inspect with `story show` and
patch frontmatter only as a last resort.
