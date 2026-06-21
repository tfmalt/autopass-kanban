use crate::config::*;
use crate::prelude::*;

/// Create a temporary repository with a minimal realistic fixture for tests
/// that previously depended on the external `../ip-2.0` checkout.
///
/// Returns `(TempDir, PathBuf)` — keep the `TempDir` alive so the temp directory
/// is not deleted before the test finishes.
pub(crate) fn build_fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir for fixture");
    let root = dir.path().to_path_buf();

    init_config(&root).unwrap();

    // Create phase subdirectories
    let phase_dir = root.join("delivery/backlog/phase-1-scaffolding");
    let cicd_dir = phase_dir.join("02.cicd-og-gitops");
    let kanban_dir = phase_dir.join("06.git-driven-kanban-and-backlog-tooling");
    fs::create_dir_all(&cicd_dir).unwrap();
    fs::create_dir_all(&kanban_dir).unwrap();

    // Epic: EP-F1-06 - Git-driven kanban and backlog tooling
    fs::write(
        kanban_dir.join("EP-F1-06-git-driven-kanban-and-backlog-tooling.md"),
        r#"---
id: EP-F1-06
type: epic
status: draft
phase: 1
owner: Solution Architect / Product Owner
milestone: MP1
created: 2026-05-28T14:05:54+0200
updated: 2026-06-11T14:08:39+0200
priority: 60
---

# Epic: Git-driven kanban and backlog tooling

---

## Acceptance Criteria

- [ ] The current sprint can be understood from markdown files and folder tree
      alone

---

## Child User Stories

| Story ID | Title | Status | Points |
|----------|-------|--------|--------|
| US-F1-052 | Add read-only CLI for sprint and backlog inspection | Done | 5 |

---
"#,
    )
    .unwrap();

    // Story: US-F1-052 (belongs to EP-F1-06, sprint S000.getting-started, status done)
    fs::write(
        kanban_dir.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md"),
        r#"---
id: US-F1-052
type: user-story
status: done
epic: EP-F1-06
sprint: S000.getting-started
assignee: TBD
story_points: 5
work_started:
work_done: 2026-05-28T22:06:38+0200
created: 2026-05-28T14:05:54+0200
updated: 2026-05-28T22:06:38+0200
---

# User Story: Add read-only CLI for sprint and backlog inspection

---

## Acceptance Criteria

**Scenario 1:** Current sprint summary is available from the CLI

---
"#,
    )
    .unwrap();

    // Tasks for US-F1-052
    fs::write(
        kanban_dir.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.tasks.md"),
        "# Tasks for US-F1-052\n\nParent User Story: US-F1-052\nSprint: S000.getting-started\n\n## TASK-US-F1-052-001 - Add shared sprint inspection summaries\n\nStatus: done\nTags: core\n\nDescription:\nExtend core with reusable sprint discovery.\n",
    )
    .unwrap();

    // Story: US-F1-010 (the multi-task story used by several tests)
    fs::write(
        cicd_dir.join("US-F1-010-ci-pipeline-build-and-unit-tests.md"),
        r#"---
id: US-F1-010
type: user-story
status: ready
epic: EP-F1-02
sprint: ~
assignee: Test User <test@example.com>
story_points: 5
work_started: 2026-05-21T00:00:00+0200
work_done:
source_path: ../../../backlog/phase-1-scaffolding/02.cicd-og-gitops/US-F1-010-ci-pipeline-build-and-unit-tests.md
task_file: US-F1-010-ci-pipeline-build-and-unit-tests.tasks.md
created: 2026-03-30T00:00:00+0200
updated: 2026-06-17T13:05:05+0200
---

# User Story: CI pipeline with build and unit tests

---

## Acceptance Criteria

**Scenario 1:** Pull request triggers CI pipeline automatically

---
"#,
    )
    .unwrap();

    // Tasks for US-F1-010 (exactly 4 tasks)
    fs::write(
        cicd_dir.join("US-F1-010-ci-pipeline-build-and-unit-tests.tasks.md"),
        "# Tasks for US-F1-010\n\nParent User Story: US-F1-010\nSprint: S001.scaffolding-part-1\n\n## TASK-US-F1-010-001 - Map how AutoPASS 1.0 CI/CD works today\n\nStatus: todo\nTags: discovery\n\nDescription:\nFind out how the current system builds and deploys.\n\n## TASK-US-F1-010-002 - Decide CI/CD platform and write ADR\n\nStatus: todo\nTags: adr\n\nDescription:\nDocument the decision in an ADR.\n\n## TASK-US-F1-010-003 - Clarify Terraform state storage location\n\nStatus: todo\nTags: terraform\n\nDescription:\nWhere does Terraform state live?\n\n## TASK-US-F1-010-004 - Document Quarkus native build approach in CI\n\nStatus: in-progress\nTags: quarkus\n\nDescription:\nFormalize the CI pipeline spec.\n",
    )
    .unwrap();

    // Sprint file for S000.getting-started (referenced by US-F1-052)
    fs::create_dir_all(root.join("delivery/sprints")).unwrap();
    fs::write(
        root.join("delivery/sprints/S000.getting-started.md"),
        "---\nsprint: S000\ntitle: Getting Started\nheadline: Getting Started\nstart_date: 2026-05-28\nend_date: 2026-06-12\nstatus: closed\nwip_limit: null\n---\n\n# S000: Getting Started\n\n## Sprint Goal\n\nKeep the team aligned on a visible sprint outcome.\n",
    )
    .unwrap();

    (dir, root)
}

pub(crate) fn init_temp_repo(temp_root: &Path) {
    init_config(temp_root).unwrap();
    fs::create_dir_all(temp_root.join("delivery/backlog")).unwrap();
    fs::create_dir_all(temp_root.join("delivery/sprints")).unwrap();
}

pub(crate) fn write_git_config(repo_root: &Path, name: &str, email: &str) {
    let init_status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("init")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(init_status.success());
    let name_status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("config")
        .arg("user.name")
        .arg(name)
        .status()
        .unwrap();
    assert!(name_status.success());
    let email_status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("config")
        .arg("user.email")
        .arg(email)
        .status()
        .unwrap();
    assert!(email_status.success());
}

pub(crate) fn sprint_readme(
    sprint: &str,
    headline: &str,
    start: &str,
    end: &str,
    status: &str,
) -> String {
    format!(
        "---\nsprint: {sprint}\nheadline: {headline}\nstart_date: {start}\nend_date: {end}\nstatus: {status}\nwip_limit: null\n---\n\n# {sprint}: {headline}\n\n## Sprint Goal\n\nKeep the team aligned on a visible sprint outcome.\n"
    )
}

pub(crate) fn write_story(temp_root: &Path, relative_path: &str, frontmatter: &str) -> PathBuf {
    let relative_path = relative_path
        .strip_prefix("doc/backlog/")
        .map(|path| format!("delivery/backlog/{path}"))
        .unwrap_or_else(|| relative_path.to_string());
    let path = temp_root.join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        format!("---\n{frontmatter}---\n\n# User Story: Test story\n\n## Acceptance Criteria\n\nScenario 1\n"),
    )
    .unwrap();
    path
}

pub(crate) fn write_story_with_task_file(
    temp_root: &Path,
    relative_path: &str,
    frontmatter: &str,
) -> PathBuf {
    let path = write_story(temp_root, relative_path, frontmatter);
    fs::write(
        path.with_extension("tasks.md"),
        "# Tasks for US-F1-001\n\nParent User Story: US-F1-001\nSprint: S001.foundation\n\n## TASK-US-F1-001-001 - First task\n\nStatus: todo\nTags: cli\n\nDescription:\nInitial work.\n",
    )
    .unwrap();
    path
}

pub(crate) fn write_sprint_file(
    temp_root: &Path,
    sprint_name: &str,
    headline: &str,
    start: &str,
    end: &str,
    status: &str,
) -> PathBuf {
    let path = temp_root.join(format!("delivery/sprints/{sprint_name}.md"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let sprint_id = sprint_name.split('.').next().unwrap();
    fs::write(
        &path,
        sprint_readme(sprint_id, headline, start, end, status),
    )
    .unwrap();
    path
}
