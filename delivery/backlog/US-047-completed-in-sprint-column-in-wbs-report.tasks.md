# Tasks for US-047

Parent User Story: US-047
Sprint: ~

## TASK-US-047-001 - Derive completion sprint in Rust report model

Status: done
Tags: report, rust

Description:
Added completed_in_sprint to ReportWorkbookRowDto; terminal stories use retained sprint; epic work_done resolves against inclusive sprint ranges; unresolved terminal data gets notes. Added Rust scenario tests.

## TASK-US-047-002 - Render completion sprint in Excel workbook

Status: done
Tags: report, python

Description:
Added Completed In Sprint column O, shifted Notes to P, updated widths, writers, styles, headers, CLI report help, and Legend & Guide semantics.

## TASK-US-047-003 - Add report model and workbook tests

Status: done
Tags: test, verification

Description:
Added Rust tests for stories, dropped stories, missing data, epic inclusive boundaries, unresolved epics, plus Python workbook and legend tests. Generated /tmp/us047-wbs-report.xlsx and inspected headers/cell placement.
