---
id: US-049
type: user-story
status: in-progress
epic: EP-007
sprint: S001.rolling-thunder
assignee: Geir Ivar Jerstad <geir.ivar.jerstad@vegvesen.no>
story_points: 5
work_started: 2026-10-23T14:38:28+0200
work_done: 
created: 2026-10-23T14:38:28+0200
updated: 2026-10-23T14:38:28+0200
activated: 2026-10-23T14:38:28+0200
---

# User Story: Create epics and relocate stories safely

---

## Story Statement

**As a** backlog maintainer,
**I want** first-class Epic creation and physical story relocation,
**so that** component sorting does not require manual file, parent or link repair.

---

## Background

The AutoPASS triage migration exposed missing commands and was undone at the
user's request. Product migration is paused until supported tooling is available.
This story belongs to EP-007, not the older hardening Epic that excludes new
commands. Rust/core owns semantics; the CLI only parses and renders.

---

## Acceptance Criteria

**Scenario 1: Create a canonical Epic**

Given an initialized repository with Epics enabled and a canonical Epic template,
when `epic create` receives a title, owner, milestone and optional id/phase,
then it creates one unique Epic with local timestamps and a draft status.
Phased repositories require one existing phase and use a numbered Epic folder;
phase-disabled repositories use the flat backlog. Duplicate/unsafe identifiers,
inconsistent phases and invalid scalar metadata fail before writes.

**Scenario 2: Relocate without losing scope or lifecycle**

Given one canonical story and one destination Epic, when `story relocate` runs,
then its file and sibling/custom task logs move beside that Epic, its parent and
updated timestamp change, and all other metadata and acceptance text remain.
Stable IDs survive cross-phase moves. An already-correct placement is a no-op.

**Scenario 3: Repair affected relationships**

Given child lists and local Markdown references, when relocation succeeds,
then source/destination membership, incoming/outgoing references and sprint
links resolve to the new locations. YAML lists, reference links and code blocks
are parsed rather than blindly replaced. Existing active child work makes a
new draft destination Epic in-progress without changing child statuses.

**Scenario 4: Reject or recover safely**

Given destination collisions, unsafe task paths, missing/duplicate parents,
invalid parent YAML or concurrent edits, when relocation is attempted,
then canonical files remain unchanged. A simulated write failure restores
original bytes and removes newly written files; rollback failures are explicit.

**Scenario 5: Supported human and JSON commands**

Given CLI users or automation, when the new commands execute,
then help describes their write effects and typed JSON envelopes expose ids,
parent/path changes and changed files using the existing error-code contract.

---

## Non-Functional Requirements

Use the existing per-repository lock and atomic writers. Preserve human-authored
content and reject path escapes. Preflight all affected writes; do not overwrite
concurrent edits or silently ignore rollback failures. Per-file atomicity and
write-error rollback do not promise a crash-atomic multi-file transaction.

---

## Technical Notes

- **Requirement refs:** EP-007 and the explicit user tooling requirement.
- **Scenarios:** the five scenarios above are the acceptance contract.
- **Component / Module:** core Epic creation and relocation; thin CLI dispatch/DTOs.
- **Parsing:** pulldown-cmark offsets, percent-encoding and serde_yaml_ng list parsing.
- **Testing approach:** core fixtures, JSON CLI integration, full workspace gates.
- **Compatibility:** `story move` remains a status transition; `story update
  --epic` remains metadata-only. Structural movement uses `story relocate`.
- **Template resolution:** `templates/TEMPLATE-epic-en.md` at repository root,
  or the configured backlog's `templates/TEMPLATE-epic-en.md`.
- **Boundaries:** repair local Markdown links, not literal paths inside code
  examples, historical prose, external links or generated/non-Markdown files.
- **Implementation plan:** core types/mutations and safety fixtures; CLI parser,
  dispatch and typed output; integration tests/docs; containerized gates/version.

### Estimation Rules

Frontmatter is the metadata source of truth. Do not duplicate frontmatter fields
in a `## Metadata` section inside the story body.

`story_points` is the only estimation field stored in frontmatter. During human
drafting it may temporarily use either a numeric Fibonacci value or a T-shirt
alias.

| T-shirt size | Story points |
| ------------ | ------------ |
| `XXS`        | `1`          |
| `XS`         | `2`          |
| `S`          | `3`          |
| `M`          | `5`          |
| `L`          | `8`          |
| `XL`         | `13`         |
| `XXL`        | `21`         |

> The authoritative alias and allowed-value lists live in the `story_points`
> block of `.kanban/settings.json`. If they differ from this table, that file
> wins — it is what `kanban validate` enforces.

- `story_points` is mandatory on all User Stories
- default `story_points` is `5` when no different estimate has yet been agreed
- draft aliases `XXS`, `XS`, `S`, `M`, `L`, `XL`, and `XXL` are allowed during manual authoring
- tools and AI agents should normalize draft aliases to numeric Fibonacci values on first write
- the canonical persisted value in the repository is numeric `story_points`, not the T-shirt label

### Workflow Lifecycle Fields

- `assignee` is a standard frontmatter field on all User Stories; use `Name <email>` when known
- `created`, `updated`, `activated`, `work_started`, and `work_done` use full local ISO 8601 timestamps with numeric timezone offset (for example `2026-05-28T14:05:54+0200`)
- `work_started` stays empty when a story is created
- set `work_started` the first time the story moves from `todo` to `in-progress`
- planning a story into a sprint normally moves it to `planned`; move it to `todo` when it is ready for execution
- preserve `work_started` if the story moves back, is blocked, or carries over to
  a new sprint
- set `work_done` when the story moves to `done`

---

## Definition of Done

- [x] Core fixtures cover creation, preservation, link repair and failure safety.
- [x] CLI acceptance test proves creation, relocation, idempotence and rejection.
- [x] Formatting, workspace tests, Clippy and build results recorded.
- [x] Backlog validation and pre-existing Doctor findings recorded.
- [x] Required workspace/skill versions updated; no product backlog migrated.
- [ ] User acceptance remains separate from executable verification.

### Verification Evidence

- All 214 core tests pass, including three creation and eight relocation tests.
- JSON CLI acceptance creates a phased Epic, relocates a story, preserves its
  body, retries idempotently and rejects duplicate creation.
- Formatting, strict workspace Clippy and `cargo build --locked` pass in the
  temporary Rust/Node/Git container bound to this exact checkout.
- An unrestricted workspace test run fails on four existing web-process tests:
  `terminate_process_stops_process_group`,
  `force_kill_process_stops_term_resistant_process_group`,
  `web_status_reports_recorded_running_port` and
  `web_status_json_reports_recorded_running_port`. Each failure reproduces on
  untouched HEAD in the same environment. All remaining workspace tests pass
  with those four explicitly excluded; two unrelated tests remain ignored by
  their existing annotations. This is not a fully green unrestricted test run.
- A disposable snapshot of the actual 308-story AutoPASS backlog passes four
  Epic creations and all ten P04 relocations using only the built CLI. Exact
  story IDs, complete bodies, non-parent lifecycle/planning fields and two task
  logs are preserved. Validation passes and Doctor gains no findings. The real
  product checkout remains clean and unmigrated after the E01 rollback.
- Tooling validation passes with 50 stories. Existing unsprinted active stories
  US-007, US-010 and US-F1-001, plus stale sprint dates, remain outside scope.
- Workspace and developer-plugin versions are `26.10.2301`. No global binary
  replacement, runtime-container replacement, commit or push is included.

---

## Dependencies

Existing repository locks, atomic writers, path containment and template helpers.
The host has no Rust compiler; verification uses the exact checkout bind-mounted
in a temporary Rust/Node/Git test image. No production/runtime image is replaced.

---

## Sprint Task Log Guidance

> Sprint execution tasks are tracked in a sibling `.tasks.md` file when this
> story is activated into a sprint. Keep that file lightweight.

Expected task log structure:

- `# Tasks for <US-ID>` file heading with optional lightweight context lines
- task heading with a lightweight task ID and verb-first title
- `Status:` using canonical workflow keywords such as `todo`, `in-progress`, `blocked`, or `done`
- `Tags:` with short labels
- `Description:` with a short note about the concrete work being done
- no `---` separators; tasks are delimited by the next `## TASK-...` heading

Keep detailed requirements, acceptance criteria, testing expectations, and
implementation guidance in this User Story rather than duplicating them in a
separate task specification document.

---

## Notes and Open Questions

No unresolved language, ownership or command-shape decisions block implementation.
The product's ten-story migration remains rolled back and must not be resumed
as a side effect of implementing this story.

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
