---
id: EP-007
type: epic
status: in-progress
phase: 1
owner: Tooling maintainer
milestone: MP2
created: 2026-10-23T14:38:28+0200
updated: 2026-10-23T14:38:28+0200
work_started: 2026-10-23T14:38:28+0200
---

# Epic: First-class structural backlog operations

## Business Context

The AutoPASS triage exercise exposed missing CLI support for Epic creation and
physical story relocation. The user requires these as supported operations,
not caller scripts. The attempted product-backlog migration was undone before
this tooling work. This Epic is bootstrapped manually because the existing
binary cannot create Epics; it is not a product-backlog migration.

## Business Value

Safe structural changes preserve human-authored Markdown and eliminate manual
parent, task-file and link repair from normal backlog workflows.

## Users and Stakeholders

CLI users and AI backlog maintainers need explicit, deterministic write commands.

## Scope

### In Scope

- `kanban epic create` with validated metadata, configured layout and templates.
- `kanban story relocate <id> --epic <id>` with physical relocation, sibling
  task logs, parent membership, local Markdown references and sprint links.
- Shared core ownership, repository locks, path containment and failure safety.

### Out of Scope

- Re-running the product triage migration, changing acceptance criteria or dates.
- New web UI/API, status vocabulary, database or unrelated tooling fixes.

## Acceptance Criteria

- [ ] Epic creation rejects duplicate IDs, invalid input and unsafe paths.
- [ ] Relocation preserves story IDs, acceptance text and lifecycle metadata.
- [ ] Parent lists, task logs and local references follow relocated stories.
- [ ] Preflight failures leave canonical files unchanged.
- [ ] Human and JSON CLI output expose the supported operations.

## Non-Functional Requirements

Use existing core locks and atomic-write helpers. Preserve unrelated Markdown,
honor optional features and do not overwrite destinations or follow unsafe paths.

## Architecture Considerations

Domain semantics belong in `crates/core`; CLI parsing/output belongs in
`crates/cli`. Existing `story move` remains a status transition and
`story update --epic` remains an explicitly documented metadata-only update.
Structural movement has its own `relocate` command, not an overloaded status.

## Dependencies

Existing repository locks, path containment, frontmatter updates and templates.

## Child User Stories

US-049: create Epics and relocate stories through first-class CLI operations.

## Definition of Done (Epic Level)

- [ ] Core and CLI regression tests cover acceptance and rejection paths.
- [ ] Formatting, workspace tests, Clippy and build pass, or blockers are recorded.
- [ ] Backlog validation/doctor results and version updates are recorded.

## Notes and Open Questions

Command names and crate ownership are resolved by the user request and existing
repository conventions. Phase-disabled repositories must remain supported.
Rust is not on the current host PATH; use the repository's Rust container image
with a bind mount of this exact checkout for executable verification.