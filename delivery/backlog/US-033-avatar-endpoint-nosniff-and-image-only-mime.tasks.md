# Tasks for US-033

Parent User Story: US-033
Sprint: ~

## TASK-US-033-001 - Restrict avatar route to image MIME with nosniff

Status: done
Tags: web-server, security

Description:
Avatar handler now rejects non-image content and sets X-Content-Type-Options: nosniff on successful image responses and rejected avatar responses.
