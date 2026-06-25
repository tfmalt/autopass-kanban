# Tasks for US-016

Parent User Story: US-016
Sprint: ~

## TASK-US-016-001 - Sanitize propagated web errors

Status: done
Tags: web-server, security

Description:
ApiResponse::from(anyhow::Error) now logs the full error chain to stderr and returns a generic 'internal error' client message, preserving explicit 400/404 responses.
