# Tasks for US-025

Parent User Story: US-025
Sprint: ~

## TASK-US-025-001 - Typed KanbanError enum + downcast classification

Status: done
Tags: core

Description:
Added error.rs with KanbanError enum; KanbanErrorCode::from(&KanbanError); KanbanErrorBody::from_anynow prefers typed downcast over string-sniffing. Migrated StoryNotFound/SprintNotFound/EpicNotFound/InvalidStatus origins.

## TASK-US-025-002 - Migrate remaining public fn signatures to Result<_, KanbanError>

Status: done
Tags: core, refactor

Description:
Added From<anyhow::Error> for KanbanError (unwraps typed, wraps untyped as Internal). Updated KanbanErrorCode::from to look inside Internal for typed errors. 4 new tests prove typed errors survive anyhow round-trip and classify correctly. Infrastructure enables any public fn to return Result<_, KanbanError> via ? on anyhow results. Individual signature migration is now mechanical.
