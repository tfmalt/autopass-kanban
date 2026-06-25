# Tasks for US-017

Parent User Story: US-017
Sprint: ~

## TASK-US-017-001 - Validate sprint route name before filesystem join

Status: done
Tags: web-server, security

Description:
Added local sprint-name grammar validator in sprint_io.rs and applied it to both the incoming route name and derived rename target before any join/rename.
