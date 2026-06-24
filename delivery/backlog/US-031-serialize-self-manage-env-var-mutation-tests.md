---
id: US-031
type: user-story
status: draft
epic: EP-003
sprint: ~
assignee: Thomas Malt <thomas.malt@vegvesen.no>
story_points: 1
work_started:
work_done:
created: 2026-06-24T08:55:41+0200
updated: 2026-06-24T08:55:41+0200
---

# User Story: Serialize self_manage environment-variable mutation tests

---

## Story Statement

**As a** maintainer,
**I want** the `self_manage` tests that mutate `GITHUB_LATEST_TAG` to run
serialized,
**so that** concurrent `set_var`/`remove_var` under the default test runner
cannot race and flake.

---

## Background

`crates/cli/src/self_manage.rs:360-380` uses `unsafe { std::env::set_var(...) }`
in two tests that run concurrently. Rust 2024 makes `set_var` unsafe because it
is not thread-safe with concurrent `env::var` reads.

**Complexity: simple** — serialize the tests or stub via a trait.

---

## Acceptance Criteria

**Scenario 1: Env-mutation tests do not race**

```gherkin
Given `cargo test` runs with default parallelism
When the two `GITHUB_LATEST_TAG` tests run
Then they do not race (serialized via a mutex or `#[serial]`)
```

**Scenario 2: No `unsafe set_var` in tests**

```gherkin
Given the refactored tests
When inspecting them
Then env mutation is either serialized or replaced by a stubbed resolver trait
```

---

## Non-Functional Requirements

| Area                | Requirement                                                          |
| ------------------- | -------------------------------------------------------------------- |
| **Test reliability** | No env-var mutation races under the parallel test runner              |

---

## Technical Notes

- **Requirement refs:** `EP-003#acceptance-criteria`
- **Component / Module:** `crates/cli/src/self_manage.rs`
- **Suggested patterns:** Use a `Mutex<()>` guard in each test, or introduce a `LatestVersionResolver` trait with a test stub. Add `serial_test` crate or a local `Mutex`.
- **Testing approach:** The tests themselves; run `cargo test` repeatedly to confirm no flake.

### Estimation Rules

`story_points` is `1` (complexity: simple).

### Workflow Lifecycle Fields

- `created` and `updated` set on authoring; `work_started` set on first move to `in-progress`.

---

## Definition of Done

- [ ] Env-mutation tests serialized or stubbed
- [ ] `cargo test` passes reliably across repeated runs
- [ ] `cargo fmt --all -- --check`, `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build` pass
- [ ] `kanban validate .` and `kanban doctor .` pass
- [ ] Workspace version bumped in `Cargo.toml`

---

## Dependencies

| Dependency                              | Type    | Status    | Notes                                  |
| --------------------------------------- | ------- | --------- | -------------------------------------- |
| None                                    | -       | -         | Standalone                             |

---

## Sprint Task Log Guidance

Expected tasks once activated:

- Add serialization guard or stub trait
- Update the two env-mutation tests

---

## Notes and Open Questions

| #   | Question / Assumption                                            | Owner        | Due        | Resolved |
| --- | ---------------------------------------------------------------- | ------------ | ---------- | -------- |
| None | - | - | - | -|

---

_Template version: 1.0 (2026-06-21) — Project-agnostic User Story template derived from the kanban tooling conventions_
