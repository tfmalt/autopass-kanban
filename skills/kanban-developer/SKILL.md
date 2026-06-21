---
name: kanban-developer
description: >
  Kanban tooling implementation workflow. Use this skill when implementing or
  continuing a `US-*`, fixing code tied to a backlog story, adding tests for a
  story, deciding how to organize new source under `crates/` or `web/`, or
  resolving implementation preflight questions. Trigger on: "continue
  implementation", "implement US-*", "start work on US-*", "develop",
  "build", "fix", "debug", "add tests", "refactor", or any coding task that
  should follow the backlog and tooling conventions in this repo. Always use
  this skill when implementation is anchored in a User Story file and tracked
  through the kanban CLI used both as the tool under development and as the
  project tracker.
isolation: worktree
---

# Kanban Developer — Kanban Tooling Workspace

Use this skill for implementation work in this repository. Its purpose is to
keep code changes anchored in the tooling architecture, backlog conventions, and
Rust/TypeScript best practices.

This skill complements:

- `kanban-backlog-maintainer` for sprint/task management and backlog authoring

## Feature lifecycle (where this skill fits)

Features move through a predictable arc. `kanban-backlog-maintainer` owns
authoring and board operation; this skill owns implementation. The story you are
handed is the *output* of steps 1–4 — treat its acceptance criteria as the
contract those steps produced.

1. **Scope** — a product need maps to an epic.
2. **Author the epic** — scope boundaries and epic-level acceptance criteria.
3. **Decompose into stories** — vertical slices with testable acceptance criteria.
4. **Plan into a sprint** — story planned, tasks created.
5. **Implement** — *this skill*: preflight, plan the change, build the smallest
   valid slice, verify against acceptance criteria.
6. **Verify and retrofit** — done-gate every criterion, then update the story to
   reflect what was actually built.

If you arrive and steps 1–4 are incomplete (no story, vague acceptance criteria,
no sprint placement), hand back to `kanban-backlog-maintainer` rather than
inventing scope while coding.

---

## Repository Architecture

This is the kanban CLI tooling workspace. The tool is both the project under
development and the backlog tracker for its own work.

### Crate workspace (`crates/`)

| Crate | Path | Responsibility |
|-------|------|----------------|
| `kanban-core` | `crates/core/` | Shared parsing, domain logic, validation, write helpers |
| `kanban-cli` | `crates/cli/` | CLI binary (`kanban` / `kb`) — argument parsing, output, command orchestration |
| `kanban-web-server` | `crates/web-server/` | Embedded Rust web server used by `kanban web start` |

### Web app (`web/`)

Vite + React frontend served by the embedded web server in production. Lives
under `web/src/` with shared types in `web/shared/`.

### Key source files

- `crates/core/src/model.rs` — domain types
- `crates/core/src/markdown.rs` — frontmatter and markdown parsing
- `crates/core/src/story.rs` — story operations
- `crates/core/src/epic.rs` — epic operations
- `crates/core/src/sprint.rs` — sprint operations
- `crates/core/src/config.rs` — configuration loading
- `crates/core/src/validate.rs` — validation logic
- `crates/core/src/repository.rs` — filesystem operations
- `crates/cli/src/cli.rs` — CLI command definitions (clap)
- `crates/cli/src/render/` — terminal output rendering

### Configuration

- `.kanban/settings.json` — paths, story points, features, web config

---

## Tooling: use the kanban CLI for backlog reads and writes

The `kanban` CLI is the source-of-truth interface for understanding and updating
stories, sprints, and task logs. **Reach for it first** for both reading the
story you are about to implement and for every sprint/task state change while
you work.

Run it from the repository root. Use `cargo run -p kanban-cli -- <args>` during
development when the binary needs to be rebuilt, or `kanban` / `kb` when a
prebuilt binary is available.

| Need | Command |
|------|---------|
| Read the story (statement, AC, task summary, resolved paths) | `kanban story show <id> --format json` |
| Resolve the canonical story markdown file | `kanban story show <id> --format json` then read the returned `path` |
| See the active sprint | `kanban sprint current --format json` |
| Plan a backlog story into the sprint | `kanban story plan --sprint <S> <id>` |
| Move story status (sets lifecycle fields) | `kanban story move <id> <status> [-a "Name <email>"]` |
| Add / update task log | `kanban task add <id> ...` · `kanban task update <id> <task_id> ...` |
| Check consistency | `kanban validate --format json` · `kanban doctor --format json` |

**JSON format:** Always add `--format json` to read-only commands.

### When kanban can't do it yet (gap protocol)

The kanban tool is under active development. If an operation you need has no
supporting command (check `cargo run -p kanban-cli -- <area> --help`), do not
silently hand-edit:

1. **Flag** the missing capability.
2. **Propose** the optimal addition — domain logic in `crates/core`, command
   surface in `crates/cli`, consistent with existing naming and the
   read-only/mutating split.
3. **Implement it** if the task scope includes it. Otherwise fall back to a
   careful manual markdown edit, and run `kanban validate --format json` plus
   `kanban doctor --format json` afterward to confirm no drift.

---

## Step 1 — Identify the implementation target

Before coding, identify the governing backlog artifact.

- Prefer an explicit `US-*` from the user.
- Resolve it with `kanban story show <id> --format json` — this gives the
  statement, acceptance criteria, task summary, sprint placement, and file
  paths as structured JSON.
- Treat the resolved user story markdown file as the implementation source of
  truth. Read the file at the returned `path` before coding; do not rely on
  the JSON summary alone.
- If only an epic or feature area is given, locate the specific story with
  `kanban story list --all --format json` before implementing.
- If the target story is ambiguous, ask one short question instead of guessing.

---

## Step 2 — Mandatory implementation preflight

Start with `kanban story show <id> --format json` to load the story, its
acceptance criteria, and task summary as structured JSON, then read these
inputs before writing code:

1. Parent epic (`EP-*`) for scope boundaries and NFRs
2. Selected User Story file (`US-*`) from the canonical markdown path
3. Story dependencies and Technical Notes
4. Story Definition of Done
5. `## Notes and Open Questions` in the story and parent epic
6. `AGENTS.md` in the repository root for development rules and conventions
7. Relevant `crates/*/src/` source files for the area being changed
8. Existing tests in the affected crate

Do not treat open questions as optional context. They are implementation gates
when they affect design choices.

---

## Step 3 — Open-question gate

If an unresolved question affects any of the following, do not start coding
until it is resolved:

- implementation language (Rust vs TypeScript for a given feature)
- crate or module layout under `crates/` or `web/`
- shared core (`crates/core`) vs interface-specific (`crates/cli`,
  `crates/web-server`, `web/`) code split
- public CLI command surface or API shape
- data model or persisted format (markdown contract, JSON schema)
- security-sensitive behavior

Acceptable resolution sources:

- explicit guidance in `AGENTS.md`
- existing patterns in the codebase
- resolved note in the epic or User Story
- direct user decision in the current session

When blocked by multiple unresolved questions, ask the highest-leverage one
first.

---

## Step 4 — Source organization rules

For new code in this workspace:

- **Backlog semantics** go in `crates/core` — validation rules, domain types,
  markdown parsing/writing, lifecycle logic.
- **CLI commands and rendering** go in `crates/cli` — argument parsing (clap),
  command orchestration, terminal output.
- **Web server logic** goes in `crates/web-server` — HTTP endpoints, static
  file serving, API responses.
- **Frontend UI** goes in `web/` — React components, views, styles.

Prefer:

- one shared core (`crates/core`) that both `crates/cli` and
  `crates/web-server` depend on
- thin interface layers over shared parsing/domain code
- small, explicit Rust types instead of loosely structured strings
- reusing existing patterns from neighboring files

Avoid:

- duplicating parsing or validation logic across crates
- introducing new dependencies without checking `Cargo.toml` first
- creating a second source of truth outside markdown and git
- silently rewriting unrelated frontmatter, prose, or formatting in backlog
  documents

---

## Step 5 — Draft a short implementation plan

Well-crafted implementation comes from deciding the shape of the change before
typing, not from improvising mid-edit. For anything beyond a trivial one-file
fix, write a brief plan first — a few lines, proportional to the work, not a
document:

- the slice you will build and which acceptance criteria it satisfies
- which crates/modules change, and where new types or commands live (per Step 4)
- the data/markdown contract or CLI surface touched, if any
- the test(s) that will prove each acceptance criterion
- how any open question resolved in Step 3 shaped the approach

If the design is cross-cutting (a new CLI command, a markdown format change, an
API shape), record the decision in the story or `AGENTS.md` so it outlives this
session. For multi-step work, the harness plan mode is a good place to draft and
confirm the plan before editing.

This is the bridge from spec to execution: the story says *what*, the plan says
*how*, and the implementation follows the plan instead of discovering it.

---

## Step 6 — Start work in the backlog before coding

If the target `US-*` is being actively worked, update the sprint artifacts
before writing code. Do not treat this as optional housekeeping.

When starting work on a story:

- plan it into the current sprint with `kanban story plan --sprint <S> <id>`
  if not already there
- create concrete implementation tasks with `kanban task add <US-ID> ...`
  before or at the moment coding starts
- use `kanban story move <id> in-progress` when work actually starts (sets
  `work_started` on first transition)
- verify the story `status` frontmatter field is correct after the command

Use `kanban sprint current --format json` to resolve the active sprint rather
than hardcoding its name.

---

## Step 7 — Implement the smallest valid slice

Implementation expectations:

- Match the acceptance criteria and Definition of Done, not a larger imagined
  roadmap.
- Prefer the smallest correct change set.
- Keep write operations reviewable.
- Add tests tied to story acceptance criteria or representative fixtures.
- Use `kanban task add` to create concrete tasks, then `kanban task update` to
  move them through statuses as implementation progresses.
- If new work is discovered mid-implementation, add a new task rather than
  silently expanding scope.

Rust-specific practices:

- Run `cargo fmt --all -- --check` before committing.
- Run `cargo clippy --workspace --all-targets -- -D warnings` and fix all
  warnings.
- Run `cargo test` to verify all tests pass.
- Use full local ISO 8601 timestamps with numeric timezone offset for any
  timestamp fields written by the tool.

If implementation reveals a backlog or convention gap:

- update the touched story if it is clearly a documentation correction, or
- flag the gap and ask before making architectural decisions

---

## Step 8 — Verify against the story

Before marking work complete:

- verify the implemented slice against the story acceptance criteria
- run `cargo test` and confirm all tests pass
- run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`
- check that open questions affecting the implemented design are resolved
- confirm the code layout matches the crate/module conventions

### Done gate

The story's acceptance criteria and linked scenarios are its contract. Do not
move a story to `done` (or `ready-for-qa`) until **every** one is resolved —
verified with evidence, or carrying an explicit recorded disposition.

Before the `done` transition:

1. Enumerate each acceptance criterion and linked scenario from the story.
2. Map each to test evidence, the task log, or an explicit disposition.
3. **If anything is unresolved, stop — do not move the story.** Flag precisely
   which items are not covered and ask the user how to proceed.
4. Record the chosen disposition in the story, then transition with
   `kanban story move <id> done`.

---

## Step 9 — Verify, build, and update version

After implementation is complete:

1. **Verify the full workspace:**
   ```bash
   cargo fmt --all -- --check
   cargo test
   cargo clippy --workspace --all-targets -- -D warnings
   ```
2. **Build:** `cargo build`
3. **Update version** in `Cargo.toml` under `[workspace.package]` per the
   versioning scheme in `AGENTS.md`:
   - `MAJOR` is the last two digits of the current year (e.g. `26` for 2026)
   - `MINOR` is the current month without leading zero (e.g. `6` for June)
   - `PATCH` is the current day of month followed by the update count for that
     day, padded to two digits for counts 1–9
4. **Update the skill version** in `skills/kanban-developer/plugin.json` to
   match the new workspace version.
5. **Retrofit the backlog** — update the task log, drive status through
   `kanban story move`, and capture any implementation discoveries in the
   story prose.

Minimum closing check:

- `kanban validate --format json` passes for the touched story
- `kanban doctor --format json` is clean if any manual edits were made
- `work_started` was set on first entry to `in-progress`
- the sibling `.tasks.md` file reflects the real work performed
- every acceptance criterion and scenario is verified or carries an explicit
  recorded disposition before `done`

---

## Verification commands

Run from the repository root:

| Command | Purpose |
|---------|---------|
| `cargo fmt --all -- --check` | Check formatting |
| `cargo test` | Run all tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint with strict warnings |
| `cargo build` | Build the workspace |
| `cargo run -p kanban-cli -- validate .` | Validate internal backlog |
| `cargo run -p kanban-cli -- doctor .` | Run backlog doctor |

For changes that modify markdown parsing or writing behavior, also run:

- `cargo run -p kanban-cli -- validate .`
- `cargo run -p kanban-cli -- doctor .`

---

## Decision heuristics

- If the question is about how the kanban tool should behave, prefer updating
  the backlog story, `AGENTS.md`, or the tool's own conventions over leaving
  the decision only in source code.
- If the choice is local and reversible (e.g. internal function signature),
  prefer the smallest practical option matching existing code style.
- If the choice is cross-cutting (e.g. CLI command naming, markdown format
  contract, API surface), escalate early and document it in the story or
  `AGENTS.md`.
- Prefer `kanban` CLI write commands over hand-editing markdown for lifecycle
  operations — the CLI keeps paths, frontmatter, and timestamps consistent.

---

## Typical triggers

Use this skill for prompts like:

- "continue implementation of US-F1-051"
- "implement the next user story"
- "fix the parser bug in crates/core"
- "add tests for this story"
- "add a new CLI subcommand for ..."
- "how should we organize this new feature across crates/"
- "build the web dashboard slice"
- "refactor the markdown parser"

Do not use this skill for backlog board operations alone. Use
`kanban-backlog-maintainer` for sprint/task administration and backlog
authoring.
