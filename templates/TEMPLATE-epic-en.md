---
id: EP-XXX
type: epic
status: draft
phase: 1
owner: Name / Role
milestone: MP1
created: YYYY-MM-DD
updated: YYYY-MM-DD
---

# Epic: [Short, descriptive title]

---

---

## Business Context

> Describe the background and why this Epic exists. Connect it to the project's
> goals: what problem does it solve, who benefits, and why now? Keep this
> readable for non-technical stakeholders.

[2–5 sentences describing the problem domain, current pain points, and how this
Epic addresses one of the core project needs.]

---

## Business Value

> What measurable or qualitative value does completing this Epic deliver? Who
> benefits and how?

- **Primary benefit:** [e.g. Users can self-serve a previously manual process]
- **Secondary benefit:** [e.g. Reduces operational support burden on the team]
- **Risk if not done:** [e.g. Continued reliance on a legacy system approaching end-of-life]

---

## Users and Stakeholders

> Who are the primary users, affected systems, and interested parties?

| Role                        | Involvement                             |
| --------------------------- | --------------------------------------- |
| [e.g. End User]             | Primary user of the feature being built |
| [e.g. Domain Expert]        | Defines and validates business rules    |
| [e.g. System Administrator] | Operates and monitors the service       |
| [e.g. External System]      | Integrates with this component          |
| [e.g. Auditor / Regulator]  | Requires traceability of decisions      |

---

## Scope

### In Scope

- [Specific functionality, domain area, or system boundary included]
- [Include integrations, rules, data flows, APIs, or UI elements]

### Out of Scope

- [Explicitly state what is NOT covered to prevent scope creep]
- [Reference related Epics that handle adjacent concerns]

---

## Acceptance Criteria

> High-level criteria that must be met for this Epic to be considered done.
> These are not user story-level tests — they define the overall outcome from a
> business and architecture perspective.

- [ ] [Business outcome criterion, e.g. A configuration change can be deployed
      without a code release]
- [ ] [Traceability criterion, e.g. All decisions are logged as
      immutable events]
- [ ] [Integration criterion, e.g. The component exposes a documented API
      consumed by downstream services]
- [ ] [Observability criterion, e.g. All operations are traceable via
      structured logging]
- [ ] [Architecture principle criterion, e.g. No vendor-specific dependencies
      introduced without explicit ADR]

---

## Non-Functional Requirements

> Specify constraints relevant to the project's quality standards.
> These apply to all User Stories within this Epic unless overridden at story
> level.

| Area                  | Requirement                                                             |
| --------------------- | ----------------------------------------------------------------------- |
| **Performance**       | [e.g. Response time ≤ 200ms p99 under normal load]                      |
| **Availability**      | [e.g. 99.9% uptime SLA; graceful degradation under peak traffic]        |
| **Traceability**      | [e.g. Every decision must be reproducible from the event log]           |
| **Auditability**      | [e.g. All operations logged with input, output, and version]            |
| **Security**          | [e.g. Authentication via standard OAuth2/OIDC protocols]                |
| **Portability**       | [e.g. No vendor-specific APIs used in application code]                 |
| **Observability**     | [e.g. Metrics, logs and traces exported via standard protocols]         |

---

## Architecture Considerations

> Note any significant architecture decisions, patterns, or constraints relevant
> to this Epic. Reference ADRs where applicable.

- **Relevant architecture principles:** [e.g. AP-03: Separation of Concerns]
- **Key patterns in play:** [e.g. Event Sourcing, CQRS, Hexagonal Architecture]
- **ADR references:** [e.g. ADR-001: Choice of messaging infrastructure]
- **Known risks or constraints:** [e.g. Migration must preserve existing data
  integrity]

---

## Dependencies

| Dependency                  | Type     | Status      | Notes                            |
| --------------------------- | -------- | ----------- | -------------------------------- |
| [EP-XXX: Related Epic]      | Epic     | In Progress | Must complete before this starts |
| [External API / System]     | External | Confirmed   | Interface contract needed        |
| [Infrastructure / Platform] | Platform | Available   | Cluster, message broker, storage |
| [Data / Migration]          | Data     | Pending     | Historical data migration plan   |

---

## Child User Stories

> List the User Stories that deliver this Epic. Update as stories are created
> and refined.

| Story ID | Title                 | Status  | Points |
| -------- | --------------------- | ------- | ------ |
| US-XXX   | [Title of User Story] | Draft   | —      |
| US-XXX   | [Title of User Story] | Refined | 5      |

---

## Definition of Done (Epic Level)

- [ ] All child User Stories are complete and accepted
- [ ] End-to-end acceptance criteria verified in staging environment
- [ ] Architecture Decision Records updated if new decisions were made
- [ ] Integration tests cover all key flows through this Epic
- [ ] Runbook / operational documentation updated
- [ ] Product Owner sign-off received

---

## Notes and Open Questions

> Track unresolved questions, assumptions, or decisions that need follow-up.
> Remove items when resolved.

| #   | Question / Assumption                       | Owner  | Due        | Resolved |
| --- | ------------------------------------------- | ------ | ---------- | -------- |
| 1   | [e.g. Which integration protocol is supported?] | [Name] | YYYY-MM-DD | No       |
| 2   | [e.g. Confirm data retention policy]            | [Name] | YYYY-MM-DD | No       |

---

_Template version: 1.0 (2026-06-21) — Project-agnostic Epic template derived from the kanban tooling conventions_
