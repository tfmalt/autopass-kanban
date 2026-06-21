# Backlog Templates

This folder contains canonical templates for creating new backlog artifacts.

## Available templates

- `TEMPLATE-epic-en.md`
- `TEMPLATE-user-story-en.md`

## Usage workflow

1. Read the project's backlog README and milestone/phase plan first.
2. Open the parent artifact (`EP-*` for stories):
   - Epics: `delivery/backlog/phase-*/*/EP-*.md`
   - User stories: `delivery/backlog/phase-*/*/US-*.md`
3. Copy the relevant template and assign the next phase-scoped ID.
4. Fill all required sections with concrete, testable content.
5. Add/update parent-child links.

## Mandatory User Story frontmatter

All User Stories must include these frontmatter fields:

- `id`
- `type`
- `status`
- `epic`
- `sprint`
- `assignee`
- `story_points`
- `work_started`
- `work_done`
- `created`
- `updated`

Rules:

- `story_points` is mandatory and defaults to `5` if no other value is set
- frontmatter is the metadata source of truth; do not duplicate it in a `## Metadata` section
- draft `story_points` aliases `XXS`, `XS`, `S`, `M`, `L`, `XL`, and `XXL` are allowed during manual authoring (the authoritative alias and allowed-value lists are the `story_points` block in `.kanban/settings.json`)
- tools and AI agents should normalize any alias to the numeric Fibonacci value on first write
- the canonical persisted form in `story_points` is numeric Fibonacci
- `assignee` is a standard field and should use `Name <email>` when known
- store `created`, `updated`, `work_started`, and `work_done` as full local ISO 8601 timestamps with numeric timezone offset
- preserve existing date-only `created` and `updated` values in older phase backlog stories unless there is a real workflow edit that normalizes them
- `work_started` and `work_done` may be blank when the story is created
- set `work_started` when the story moves to `in-progress` for the first time
- set `work_done` when the story moves to `done`

## Mandatory Epic frontmatter

All Epics must include these frontmatter fields:

- `id`
- `type`
- `status`
- `phase`
- `owner`
- `milestone`
- `created`
- `updated`

Rules:

- epic frontmatter is the metadata source of truth; do not duplicate it in a `## Metadata` section
- `phase` stores the numeric phase identifier
- `owner` stores the responsible role or owner text
- `milestone` stores the compact milestone code
- `created` and `updated` should use full local ISO 8601 timestamps with numeric timezone offset when known; preserve existing date-only values in older artifacts unless there is a real workflow edit

## Optional frontmatter metadata

- `priority` may be added to either Epic or User Story frontmatter as an optional ordering rank.
- Use a non-negative integer when present.
- Lower numbers sort first; missing or blank `priority` values sort last.
- Do not add `priority` to template frontmatter by default. Leave it absent unless the backlog has been explicitly ranked or a tool writes it during reorder operations.

## Quality expectations

- Epic: clear scope boundaries and measurable outcomes, with canonical metadata in frontmatter only.
- User Story: testable Gherkin acceptance criteria.
- Sprint task logs: keep them lightweight, execution-focused, and attached as `.tasks.md` beside active sprint stories.

## Backlog naming and structure checklist

Use this quick checklist when creating new backlog artifacts.

- Place artifacts in epic-centric folders under the correct phase:
  - `delivery/backlog/phase-{n}-.../{epicNo}.{epic-slug}/`
- Keep parent and children together in the same epic folder:
  - `EP-{id}-{slug}.md`
  - `US-{id}-{slug}.md`
- Use lowercase ASCII slugs with hyphens only (`a-z`, `0-9`, `-`).
- Keep slugs short and outcome-focused.
- Keep language consistent within a phase (Norwegian or English).
- Keep IDs stable and phase-scoped; never renumber historical artifacts.

### Recommended User Story ID allocation by epic

Within each phase, reserve story ID blocks by epic number:

- First epic in phase -> stories with lower IDs
- Second epic in phase -> stories with the next block of IDs
- Continue the same pattern for additional epics

Notes:

- Gaps are fine; do not backfill by renumbering.
- If an epic block is full, use the next available phase-scoped ID and note
  the exception in the parent epic.

## Important

- Markdown is authoritative.
- Sprint tooling must never become the only place where workflow state exists.
