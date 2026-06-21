---
id: US-XXX
type: user-story
status: draft
epic: EP-XXX
sprint: ~
assignee: Name <email@example.com> # or comma-separated list for shared ownership
story_points: 5
work_started:
work_done:
created: YYYY-MM-DDTHH:MM:SS+HHMM
updated: YYYY-MM-DDTHH:MM:SS+HHMM
---

# User Story: [Short, outcome-focused title]

---

## Story Statement

> The core of the User Story. Keep it concise and outcome-focused.
> Use the standard format, choosing the most accurate user role.

**As a** [developer / operator / end user / system administrator / auditor],
**I want** [a specific capability or behaviour],
**so that** [the business or user value this delivers].

---

## Background

> 2–4 sentences providing the "why" behind this story. What is the
> current state, what problem does this solve, and how does it fit
> into the broader Epic? Useful for developers picking up the story
> cold and for AI assistants generating implementation.

[Contextual description. For example: "Currently, configuration changes require
a code change and a production deployment. This story introduces a configuration
management module so that operators can update settings independently via the
admin UI."]

---

## Acceptance Criteria

> Written in [Gherkin](https://cucumber.io/docs/gherkin/) (Given/When/Then) format — a structured specification
> language from [Behaviour-Driven Development (BDD)](https://cucumber.io/docs/bdd/).
> Each criterion should be independently verifiable and unambiguous. Aim for 3–7 criteria.

**Scenario 1: [Descriptive name of the scenario]**

```gherkin
Given [initial context or system state]
When  [action or event that occurs]
Then  [expected observable outcome]
```

**Scenario 2: [Descriptive name]**

```gherkin
Given [context]
When  [action]
Then  [outcome]
```

**Scenario 3: [Edge case or error scenario]**

```gherkin
Given [context for edge case]
When  [triggering action]
Then  [expected safe or meaningful response]
```

> Add more scenarios as needed. Include at least one error/edge case scenario.

---

## Non-Functional Requirements

> Specify any requirements that go beyond functional correctness.
> Inherit from parent Epic unless explicitly overridden here.

| Area               | Requirement                                                           |
| ------------------ | --------------------------------------------------------------------- |
| **Performance**    | [e.g. Response time ≤ Xms at p99; throughput ≥ N req/s]               |
| **Traceability**   | [e.g. Each operation must be logged with input, output, version]      |
| **Auditability**   | [e.g. Decision log must be queryable for the retention period]        |
| **Security**       | [e.g. Endpoint requires authenticated JWT with role claim X]          |
| **Observability**  | [e.g. Span created for each operation, exported via standard protocol]|
| **Portability**    | [e.g. No vendor-specific SDK calls in implementation]                 |

---

## Technical Notes

> Guidance for developers and AI assistants on expected implementation
> approach. This section is non-prescriptive — teams can deviate with
> justification. Include relevant architecture patterns, module hints,
> or integration points.

- **Requirement refs:** [e.g. `REQ-02#req02-...`, `REQ-04#req04-...`]
- **Acceptance criteria refs:** [e.g. `AC-REQ02-...`, `AC-REQ04-...`]
- **Scenarios:** [e.g. `SC-CREATE-01`, `SC-UPDATE-02`]
- **Feature tokens:** [e.g. `feature-name`]
- **Component / Module:** [e.g. `module-name` / `sub-module`]
- **Key integration points:** [e.g. Consumes message topic `events.v1`; calls downstream service via REST/gRPC]
- **Suggested patterns:** [e.g. Repository pattern for data access; event sourcing for state changes]
- **Data model hints:** [e.g. Extend `Entity` with new fields; decisions stored as immutable events]
- **Testing approach:** [e.g. Unit test core logic; integration test against embedded dependencies; contract test API]
- **Migration / backward compatibility:** [e.g. Existing events must remain readable; no breaking API changes]

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
| [US-XXX: Blocking story]    | Story          | Done      | Must be complete before implementation |
| [Infrastructure component]  | Infrastructure | Available | Schema documented                      |
| [External service / API]    | External       | Confirmed | Interface contract agreed              |
| [Configuration / Data]      | Data           | Pending   | Awaiting domain expert sign-off        |

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
| 1   | [e.g. Should failed operations be retried?]           | [Name]  | YYYY-MM-DD | No       |
| 2   | [e.g. Confirm retention period for event log]         | [Name]  | YYYY-MM-DD | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
