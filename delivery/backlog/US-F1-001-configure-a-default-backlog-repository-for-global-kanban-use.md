---
id: US-F1-001
type: user-story
status: in-progress
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 5
work_started: 2026-08-26T17:16:00+0200
work_done:
created: 2026-08-26T17:14:18+0200
updated: 2026-08-26T17:16:00+0200
---

# User Story: Configure a default backlog repository for global kanban use

---

## Story Statement

> The core of the User Story. Keep it concise and outcome-focused.
> Use the standard format, choosing the most accurate user role.

**As a** developer working across several Git repositories,
**I want** to configure a default repository that owns my kanban backlog,
**so that** I can run `kanban` from an unrelated code repository without
repeating the backlog path.

---

## Background

> 2–4 sentences providing the "why" behind this story. What is the
> current state, what problem does this solve, and how does it fit
> into the broader Epic? Useful for developers picking up the story
> cold and for AI assistants generating implementation.

Every operational command currently treats the current directory as its target
repository. In a polyrepo workspace this makes the CLI unusable from service
repositories that do not contain the backlog. The selected backlog must remain
explicit, deterministic, and safe for mutations.

---

## Acceptance Criteria

> Written in [Gherkin](https://cucumber.io/docs/gherkin/) (Given/When/Then) format — a structured specification
> language from [Behaviour-Driven Development (BDD)](https://cucumber.io/docs/bdd/).
> Each criterion should be independently verifiable and unambiguous. Aim for 3–7 criteria.

**Scenario 1: Configured default serves an unrelated repository**

```gherkin
Given a globally configured Git repository with `.kanban/settings.json`
And I am in an unrelated Git repository without `.kanban/settings.json`
When I run a repository command without `REPO_ROOT`
Then the command reads or writes the configured backlog repository
```

**Scenario 2: Local and explicit repositories remain predictable**

```gherkin
Given the default backlog root is configured
When I run a command inside a different repository that has `.kanban/settings.json`
Then that local repository is used
And an explicit `REPO_ROOT` or `KANBAN_REPO_ROOT` overrides both defaults
```

**Scenario 3: Invalid configured roots do not redirect writes**

```gherkin
Given the configured default root no longer identifies an initialized Git repository
When I run a repository command without `REPO_ROOT`
Then the command fails with actionable configuration guidance
And it does not fall back to the current directory
```

> Add more scenarios as needed. Include at least one error/edge case scenario.

---

## Non-Functional Requirements

> Specify any requirements that go beyond functional correctness.
> Inherit from parent Epic unless explicitly overridden here.

| Area               | Requirement                                                           |
| ------------------ | --------------------------------------------------------------------- |
| **Safety**         | Invalid configured roots fail; no silent fallback may target another repository. |
| **Compatibility**  | Existing behavior is unchanged when no global root is configured.     |
| **Portability**    | User configuration follows XDG with an environment override.          |

---

## Technical Notes

> Guidance for developers and AI assistants on expected implementation
> approach. This section is non-prescriptive — teams can deviate with
> justification. Include relevant architecture patterns, module hints,
> or integration points.

- **Component / Module:** `crates/core` user configuration and root selection; `crates/cli` argument and command orchestration.
- **Configuration:** `${KANBAN_CONFIG_HOME:-${XDG_CONFIG_HOME:-~/.config}/kanban}/config.json` stores an optional canonical absolute `default_repo_root`.
- **Precedence:** explicit `REPO_ROOT`, `KANBAN_REPO_ROOT`, current Git worktree when initialized for kanban, configured default, current directory.
- **Exceptions:** `kanban init`, upgrade, uninstall, completion, and global-config commands do not select the default root.
- **Testing approach:** unit-test precedence and invalid configurations; integration-test bare commands from unrelated repositories.

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

> All items below must be met before this story can be accepted.
> This list reflects project team standards.

- [ ] Acceptance criteria verified and signed off by Product Owner
- [ ] Code reviewed and approved via pull request (minimum 1 reviewer)
- [ ] Unit tests written and all pass (coverage ≥ threshold defined by team)
- [ ] Integration tests cover main acceptance criteria scenarios
- [ ] No new static analysis issues introduced (or justified exceptions documented)
- [ ] Relevant events/messages produced or consumed are documented
- [ ] Business rules peer-reviewed by domain expert (if applicable)
- [ ] API changes documented in spec (if applicable)
- [ ] Observability: spans, metrics, and structured logs in place
- [ ] No hard-coded vendor dependencies introduced
- [ ] Architecture Decision Record (ADR) created if a significant decision was made
- [ ] Story demo-ready for sprint review

---

## Dependencies

| Dependency                  | Type           | Status    | Notes                                  |
| --------------------------- | -------------- | --------- | -------------------------------------- |
| Existing repository config  | Configuration  | Available | `.kanban/settings.json` remains repository-local |

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

| #   | Question / Assumption                                 | Owner   | Due        | Resolved |
| --- | ----------------------------------------------------- | ------- | ---------- | -------- |
| 1   | Should a local initialized repository override the user default? | Tooling lead | 2026-08-26 | Yes, to preserve local behavior. |
| 2   | Should a missing configured root silently fall back to cwd? | Tooling lead | 2026-08-26 | No, fail to prevent wrong-repository writes. |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
