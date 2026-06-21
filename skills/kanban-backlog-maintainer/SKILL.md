---
name: kanban-backlog-maintainer
description: >
  Maintain and operate a markdown-based kanban backlog and sprint board using the
  `kanban` CLI. Use this skill whenever authoring or refining backlog scope —
  writing or splitting an Epic, decomposing an Epic into User Stories, drafting
  acceptance criteria, placing new work so it fits the product's existing scope —
  as well as editing, validating, planning, tracking, or reporting Epics, User
  Stories, tasks, and sprints. Trigger on "write a story/epic", "break this epic
  down", "add a backlog item", "groom/refine the backlog", or any planning that
  shapes what gets built. Use `--format json` for read-only commands and `kanban`
  CLI operations first before falling back to manual file edits.
---

# Kanban Backlog Maintainer

Maintain backlog quality and operate the sprint board via the `kanban` CLI. Use
`kanban` commands first for lifecycle, sprint, task, and reporting operations.
Fall back to direct file edits only when a `kanban` command does not yet exist
for the needed operation.

## Feature lifecycle (where this skill fits)

A feature moves through a predictable arc. This skill owns authoring and board
operation; the sibling `kanban-developer` skill owns implementation. Knowing the
whole arc keeps each artifact pointed at the next step rather than written in
isolation.

1. **Scope** — a product need maps to an epic (existing or new).
2. **Author the epic** — business context, scope boundaries, epic-level
   acceptance criteria, NFRs.
3. **Decompose into stories** — vertical slices with testable Gherkin acceptance
   criteria that together satisfy the epic's acceptance criteria.
4. **Plan into a sprint** — `kanban story plan`, then break stories into tasks.
5. **Implement** — handled by `kanban-developer`.
6. **Verify and retrofit** — prove every acceptance criterion, then update the
   story to reflect what was actually built.

This skill drives steps 1–4 and 6; step 5 is `kanban-developer`.

## Prerequisites

- The target repository must be a git repository with `.kanban/settings.json`
  (run `kanban init` from the repo root if `.kanban/` is missing).
- A `delivery/backlog/README.md` or similar project convention document should
  define the project's ID scheme, artifact layout, state model, and templates.

## Optional features

Phases, sprints, and epics are individually optional. A project that does not
use sprints should pass `--no-sprints` to `kanban init` (or run
`kanban features disable sprints` later). The same applies to `--no-epics` and
`--no-phases`. When a feature is off, the corresponding `kanban <area> *`
subcommands return a clear "feature disabled" error, the matching story
frontmatter field is no longer required, and `validate`/`doctor` skip the
feature-specific rules. Always check the `paths.features` block in
`.kanban/settings.json` before assuming a project uses sprints, epics, or
phases.

## Required Reading

Before creating or editing backlog artifacts, read in this order:

1. `delivery/backlog/README.md` — authoritative IDs, layout, workflow, and
   quality conventions (adjust the path if `paths.backlog` in
   `.kanban/settings.json` configures a different backlog directory).
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
| Toggle optional features | `kanban features list`; `kanban features enable/disable <sprints|epics|phases>` |
| WBS Excel report | `kanban --format json report wbs \| python3 scripts/wbs_report.py` |

When a requested operation has no command, check `kanban <area> --help`, name
the gap, suggest a concrete future command, then do the smallest safe manual
edit.

## Fit New Work Into The Product Scope

A story or epic earns its place only if it advances the product and does not
collide with work that already exists. Before authoring anything new, locate it
in the current backlog so it lands in the right epic, at the right phase, without
duplicating or contradicting existing scope. Skipping this is the most common way
backlogs accumulate near-duplicate stories and contradictory acceptance criteria.

1. **Survey what exists.** Run `kanban story list --all --format json` and, if
   epics are enabled, read the relevant epic files (or `kanban phase show <phase>
   --format json`). Skim the backlog README's milestone/phase plan for sequencing.
2. **Find the home.** Map the new capability to an existing epic. If nothing
   fits, the work likely needs a *new* epic — author that first, because a story
   without a coherent parent usually signals missing scope rather than a missing
   story.
3. **Check for overlap.** If an existing story already covers part of the
   capability, extend or split it instead of creating a near-duplicate. Note
   adjacent stories so their acceptance criteria do not contradict each other.
4. **Respect sequencing.** Place the work in the phase/milestone whose
   dependencies are satisfied, and record cross-story dependencies in the
   Dependencies table rather than leaving ordering implicit.
5. **Name the goal it serves.** If you cannot state which epic outcome or product
   need the story advances, treat that as a signal to clarify scope with the user
   before writing acceptance criteria.

## Workflows

### Create Or Update Epic/Story

Use the required reading, then follow the project's template exactly:

1. Place the work first (see *Fit New Work Into The Product Scope*): confirm the
   parent Epic for stories, and create the Epic first if missing.
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

### Decompose An Epic Into Stories

An epic is well-authored only when its child stories, taken together, satisfy its
acceptance criteria with no gaps and no overlap. When breaking an epic down:

- Derive stories from the epic's **acceptance criteria and scope**, not from a
  technical component list. Each story should deliver an observable slice of value
  someone could exercise (a vertical slice), not a horizontal layer like "the
  database part" that delivers nothing on its own.
- Aim for INVEST-shaped stories — independent, negotiable, valuable, estimable,
  small, testable. If a story cannot be estimated within the project's allowed
  `story_points` range, it is too big: split it along a workflow step, a data
  variant, or a happy-path/edge-case boundary.
- Cover the scope collectively. List the epic's acceptance criteria and confirm
  each one maps to at least one child story; flag any criterion with no owning
  story as a gap to fill or to move out of scope.
- Avoid overlap. Two stories should not both claim the same acceptance criterion
  or the same code surface unless there is an explicit dependency between them.
- Keep the links coherent as the set evolves: the epic's `Child User Stories`
  table and each story's `epic` field must stay in sync when stories are added,
  split, or dropped.

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
