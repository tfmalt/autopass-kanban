# Tasks for US-027

Parent User Story: US-027
Sprint: ~

## TASK-US-027-001 - Replace parent unwraps with contextual errors

Status: done
Tags: core, robustness

Description:
Replaced production parent().unwrap() sites in repository.rs and doctor.rs with parent().with_context(...)? so edge-case paths return errors instead of panicking.
