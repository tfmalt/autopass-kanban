mod config;
mod constants;
mod doctor;
mod json;
mod markdown;
mod model;
mod phase;
mod repository;
mod sprint;
mod story;
mod util;
mod validate;

pub(crate) mod prelude {
    pub(crate) use anyhow::{Context, Result, anyhow, bail};
    pub(crate) use chrono::{Datelike, Days, Local, NaiveDate, TimeZone, Weekday};
    pub(crate) use regex::Regex;
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    pub(crate) use std::fs;
    pub(crate) use std::path::{Path, PathBuf};
    pub(crate) use std::process::Command;
    pub(crate) use walkdir::WalkDir;
}

pub use config::{
    ColorMode, ConfigInitResult, ConfigSetResult, KanbanConfig, get_config_json, get_config_value,
    init_config, load_kanban_config, resolve_repo_root, set_config_value,
};
pub use constants::*;
pub use doctor::*;
pub use json::*;
pub use markdown::*;
pub use model::*;
pub use phase::*;
pub use repository::*;
pub use sprint::*;
pub use story::*;
pub use validate::*;

#[cfg(test)]
mod tests {
    use crate::config::*;
    use crate::constants::*;
    use crate::doctor::*;
    use crate::markdown::*;
    use crate::model::*;
    use crate::phase::*;
    use crate::prelude::*;
    use crate::repository::*;
    use crate::sprint::*;
    use crate::story::*;
    use crate::util::*;
    use crate::validate::*;
    use tempfile::tempdir;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../")
            .canonicalize()
            .unwrap()
    }

    fn init_temp_repo(temp_root: &Path) {
        init_config(temp_root).unwrap();
        fs::create_dir_all(temp_root.join("delivery/backlog")).unwrap();
        fs::create_dir_all(temp_root.join("delivery/sprints")).unwrap();
    }

    fn write_git_config(repo_root: &Path, name: &str, email: &str) {
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

    fn sprint_readme(sprint: &str, headline: &str, start: &str, end: &str, status: &str) -> String {
        format!(
            "---\nsprint: {sprint}\nheadline: {headline}\nstart_date: {start}\nend_date: {end}\nstatus: {status}\nwip_limit: null\n---\n\n# {sprint}: {headline}\n\n## Sprint Goal\n\nKeep the team aligned on a visible sprint outcome.\n"
        )
    }

    fn write_story(temp_root: &Path, relative_path: &str, frontmatter: &str) -> PathBuf {
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

    fn write_story_with_task_file(
        temp_root: &Path,
        relative_path: &str,
        frontmatter: &str,
    ) -> PathBuf {
        let path = write_story(temp_root, relative_path, frontmatter);
        fs::write(
            path.with_extension("tasks.md"),
            "# Tasks for US-F1-001\n\nParent User Story: US-F1-001\nSprint: S001.foundation\n\n---\n\n## TASK-US-F1-001-001 - First task\n\nStatus: To Do\nTags: cli\n\nDescription:\nInitial work.\n",
        )
        .unwrap();
        path
    }

    fn write_sprint_file(
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

    #[test]
    fn collect_user_story_files_returns_backlog_stories_but_not_task_files() {
        let repo_root = repo_root();
        let story_files = collect_user_story_files(&repo_root).unwrap();

        assert!(story_files.iter().any(|story_file| {
            story_file.ends_with("US-F1-010-ci-pipeline-build-and-unit-tests.md")
        }));
        assert!(
            !story_files
                .iter()
                .any(|story_file| story_file.to_string_lossy().ends_with(".tasks.md"))
        );
    }

    #[test]
    fn read_story_file_reads_canonical_backlog_story_and_sibling_tasks() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story_with_task_file(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-001-test-story.md",
            "id: US-F1-001\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let story = read_story_file(&story_path, temp_root.path()).unwrap();

        assert_eq!(story.sprint_name.as_deref(), Some("S001.foundation"));
        assert_eq!(
            story.frontmatter.get("status").map(String::as_str),
            Some("in-progress")
        );
        let task_file = story.task_file.as_ref().unwrap();
        assert!(task_file.exists);
        assert_eq!(task_file.tasks.len(), 1);
    }

    #[test]
    fn update_story_frontmatter_writes_requested_fields() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-099-test-story.md",
            "id: US-F1-099\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint:\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = update_story_frontmatter(
            temp_root.path(),
            "US-F1-099",
            &[
                ("status".to_string(), "ready".to_string()),
                ("story_points".to_string(), "5".to_string()),
                ("assignee".to_string(), "TBD".to_string()),
            ],
        )
        .unwrap();

        let markdown = fs::read_to_string(story_path).unwrap();
        assert_eq!(result.story_id, "US-F1-099");
        assert_eq!(
            result.updated_fields,
            vec!["status", "story_points", "assignee"]
        );
        assert!(markdown.contains("status: ready"));
        assert!(markdown.contains("story_points: 5"));
        assert!(markdown.contains("assignee: TBD"));
    }

    #[test]
    fn validate_story_accepts_single_source_story_fixture() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-010-ci-pipeline-build-and-unit-tests.md",
            "id: US-F1-010\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.scaffolding-part-1\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\nactivated: 2026-05-28T14:05:54+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        assert!(validate_story(&story).is_empty());
    }

    #[test]
    fn validate_story_allows_date_only_created_and_updated_on_draft_backlog_fixture() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-050-test-draft-story.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-050\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28\nupdated: 2026-05-28\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(!rules.contains(&"missing-field:assignee"));
        assert!(!rules.contains(&"invalid-timestamp:created"));
        assert!(!rules.contains(&"invalid-timestamp:updated"));
    }

    #[test]
    fn doctor_does_not_report_date_only_created_or_updated() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-050-test-draft-story.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-050\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28\nupdated: 2026-05-31\n---\n# User Story\n",
        )
        .unwrap();

        let issues = collect_doctor_issues_for_story(temp_root.path(), "US-F1-050").unwrap();
        assert!(
            issues
                .iter()
                .all(|issue| issue.rule != "invalid-timestamp:created")
        );
        assert!(
            issues
                .iter()
                .all(|issue| issue.rule != "invalid-timestamp:updated")
        );
        assert!(
            fs::read_to_string(&story_path)
                .unwrap()
                .contains("created: 2026-05-28")
        );
        assert!(
            fs::read_to_string(&story_path)
                .unwrap()
                .contains("updated: 2026-05-31")
        );
    }

    #[test]
    fn validate_story_allows_todo_without_assignee_even_after_work_started() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-051-build-shared-backlog-parsing-and-validation-core.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-051\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nstory_points: 5\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(!rules.contains(&"missing-field:assignee"));
    }

    #[test]
    fn validate_story_requires_assignee_when_in_progress() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-051-build-shared-backlog-parsing-and-validation-core.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-051\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nstory_points: 5\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"missing-field:assignee"));
    }

    #[test]
    fn validate_story_requires_non_empty_assignee_when_in_progress() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-051-build-shared-backlog-parsing-and-validation-core.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-051\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee:\nstory_points: 5\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"missing-field:assignee"));
    }

    #[test]
    fn validate_repository_flags_invalid_sprint_status_and_missing_task_file_after_start() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "paused",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-051-build-shared-backlog-parsing-and-validation-core.md",
            "id: US-F1-051\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ntask_file: US-F1-051-build-shared-backlog-parsing-and-validation-core.tasks.md\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let validation = validate_repository(temp_root.path()).unwrap();
        let rules: Vec<&str> = validation
            .issues
            .iter()
            .map(|issue| issue.rule.as_str())
            .collect();

        assert!(rules.contains(&"invalid-sprint-readme-status"));
        assert!(rules.contains(&"missing-task-file"));
    }

    #[test]
    fn summarize_current_sprint_uses_sprint_file_dates() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2026-05-18",
            "2026-05-29",
            "planned",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md",
            "id: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let sprint = summarize_current_sprint_at_date(
            temp_root.path(),
            NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
        )
        .unwrap();

        assert_eq!(sprint.sprint_name, "S001.foundation");
        assert_eq!(
            sprint.sprint_goal.as_deref(),
            Some("Keep the team aligned on a visible sprint outcome.")
        );
        assert_eq!(sprint.readme_status.as_deref(), Some("planned"));
    }

    #[test]
    fn summarize_current_sprint_prefers_single_active_sprint_when_dates_are_overdue() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2026-05-18",
            "2026-05-29",
            "active",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md",
            "id: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let sprint = summarize_current_sprint_at_date(
            temp_root.path(),
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
        )
        .unwrap();

        assert_eq!(sprint.sprint_name, "S001.foundation");
        assert_eq!(sprint.readme_status.as_deref(), Some("active"));
    }

    #[test]
    fn list_current_sprint_stories_returns_flattened_current_sprint_rows() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2026-05-18",
            "2026-05-29",
            "active",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md",
            "id: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 8\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let (sprint_name, stories) = list_current_sprint_stories(temp_root.path()).unwrap();

        assert_eq!(sprint_name, "S001.foundation");
        assert_eq!(stories.len(), 2);
        assert_eq!(stories[0].id, "US-F1-052");
        assert_eq!(stories[1].id, "US-F1-053");
    }

    #[test]
    fn list_next_sprint_stories_uses_next_numbered_sprint_after_current() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let today = Local::now().date_naive();
        let current_start = today.checked_sub_days(Days::new(1)).unwrap().to_string();
        let current_end = today.checked_add_days(Days::new(1)).unwrap().to_string();
        let next_start = today.checked_add_days(Days::new(2)).unwrap().to_string();
        let next_end = today.checked_add_days(Days::new(13)).unwrap().to_string();

        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            &current_start,
            &current_end,
            "active",
        );
        write_sprint_file(
            temp_root.path(),
            "S002.delivery",
            "delivery",
            &next_start,
            &next_end,
            "planned",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-054-add-cli-support-for-completing-tasks-from-the-terminal.md",
            "id: US-F1-054\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S002.delivery\nassignee: TBD\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let (sprint_name, stories) = list_next_sprint_stories(temp_root.path()).unwrap();

        assert_eq!(sprint_name, "S002.delivery");
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].id, "US-F1-054");
    }

    #[test]
    fn list_all_stories_returns_single_story_entry() {
        let repo_root = repo_root();

        let stories = list_all_stories(&repo_root).unwrap();
        let matching = stories
            .iter()
            .find(|story| story.id == "US-F1-010")
            .unwrap();
        let count = stories
            .iter()
            .filter(|story| story.id == "US-F1-010")
            .count();

        assert_eq!(count, 1);
        assert!(
            matching
                .relative_path
                .to_string_lossy()
                .contains("US-F1-010")
        );
    }

    #[test]
    fn summarize_phase_lists_backlog_stories_with_sprint_assignment() {
        let repo_root = repo_root();
        let phase = summarize_phase(&repo_root, "F1").unwrap();

        assert_eq!(phase.phase, "F1");
        assert!(phase.stories.iter().any(|story| {
            story.id == "US-F1-052" && story.sprint.as_deref() == Some("S000.getting-started")
        }));
        assert!(phase.stories.iter().any(|story| {
            story.id == "US-F1-052"
                && story.epic_id.as_deref() == Some("EP-F1-06")
                && story.epic_title.as_deref() == Some("Git-driven kanban and backlog tooling")
        }));
    }

    #[test]
    fn find_story_exposes_acceptance_criteria_and_tasks() {
        let repo_root = repo_root();
        let story = find_story(&repo_root, "US-F1-010").unwrap().unwrap();

        assert!(
            story
                .acceptance_criteria
                .as_deref()
                .unwrap_or_default()
                .contains("Scenario 1")
        );
        assert_eq!(story.tasks.len(), 4);
    }

    #[test]
    fn doctor_reports_sprint_status_disagreement_with_current_dates() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2026-05-18",
            "2026-05-29",
            "planned",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md",
            "id: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let findings = doctor_repository_at_date(
            temp_root.path(),
            NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
        )
        .unwrap();

        assert!(findings.iter().any(|finding| {
            finding.scope == "S001.foundation"
                && finding
                    .message
                    .contains("README frontmatter is authoritative")
        }));
    }

    #[test]
    fn collect_doctor_issues_for_story_targets_single_canonical_story_file() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nstory_points: 8\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let issues = collect_doctor_issues_for_story(temp_root.path(), "US-F1-053").unwrap();

        assert!(!issues.is_empty());
        assert!(
            issues
                .iter()
                .all(|issue| issue.story_id.as_deref() == Some("US-F1-053"))
        );
        assert!(
            issues.iter().any(|issue| issue.file_path.as_ref()
                == Some(&relative_path(temp_root.path(), &story_path)))
        );
    }

    #[test]
    fn apply_doctor_fix_sets_missing_assignee_on_story_file() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_git_config(temp_root.path(), "Test User", "test@example.com");
        let story_path = temp_root
            .path()
            .join("delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-051-build-shared-backlog-parsing-and-validation-core.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-051\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nstory_points: 5\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let issue = collect_doctor_issues_for_story(temp_root.path(), "US-F1-051")
            .unwrap()
            .into_iter()
            .find(|issue| issue.rule == "missing-field:assignee")
            .unwrap();
        let result =
            apply_doctor_fix(temp_root.path(), &issue, &DoctorFixInput::default()).unwrap();
        let updated = fs::read_to_string(&story_path).unwrap();

        assert!(result.message.contains("Set assignee"));
        assert!(updated.contains("assignee: Test User <test@example.com>"));
    }

    #[test]
    fn create_sprint_creates_single_file_and_readme() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        fs::create_dir_all(temp_root.path().join("delivery/sprints")).unwrap();
        let today = Local::now().date_naive();
        let input = CreateSprintInput {
            number: 1,
            start_date: today,
            end_date: today + Days::new(11),
            headline: "Foundation Sprint".to_string(),
        };

        let result = create_sprint(temp_root.path(), &input).unwrap();

        assert_eq!(result.sprint_name, "S001.foundation-sprint");
        let sprint_file = temp_root.path().join(&result.sprint_path);
        assert!(sprint_file.exists());
        let markdown = fs::read_to_string(sprint_file).unwrap();
        assert!(markdown.contains("status: planned"));
        assert!(markdown.contains(ROSTER_HEADING));
    }

    #[test]
    fn create_sprint_uses_configured_sprints_path() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "paths.sprints", "planning/sprints").unwrap();
        let today = Local::now().date_naive();
        let input = CreateSprintInput {
            number: 1,
            start_date: today,
            end_date: today + Days::new(11),
            headline: "Foundation Sprint".to_string(),
        };

        let result = create_sprint(temp_root.path(), &input).unwrap();

        assert_eq!(
            result.sprint_path,
            PathBuf::from("planning/sprints/S001.foundation-sprint.md")
        );
        assert!(
            temp_root
                .path()
                .join("planning/sprints/S001.foundation-sprint.md")
                .exists()
        );
    }

    #[test]
    fn suggested_next_sprint_dates_use_latest_sprint_file_end_date() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let sprints_root = temp_root.path().join("delivery/sprints");
        fs::create_dir_all(&sprints_root).unwrap();
        fs::write(
            sprints_root.join("S000.getting-started.md"),
            sprint_readme(
                "S000",
                "getting-started",
                "2026-05-18",
                "2026-05-29",
                "closed",
            ),
        )
        .unwrap();
        fs::write(
            sprints_root.join("S001.foundation.md"),
            sprint_readme("S001", "foundation", "2026-06-02", "2026-06-13", "planned"),
        )
        .unwrap();

        let suggestion = suggested_next_sprint_dates(temp_root.path())
            .unwrap()
            .unwrap();

        assert_eq!(suggestion.0, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
        assert_eq!(suggestion.1, NaiveDate::from_ymd_opt(2026, 6, 26).unwrap());
    }

    #[test]
    fn read_and_validate_story_use_configured_backlog_and_sprint_paths() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "paths.backlog", "planning/backlog").unwrap();
        set_config_value(temp_root.path(), "paths.sprints", "planning/sprints").unwrap();

        let sprint_file = temp_root.path().join("planning/sprints/S001.foundation.md");
        let backlog_dir = temp_root
            .path()
            .join("planning/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling");
        let story_file = "US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md";

        fs::create_dir_all(sprint_file.parent().unwrap()).unwrap();
        fs::create_dir_all(&backlog_dir).unwrap();
        fs::write(
            &sprint_file,
            sprint_readme("S001", "foundation", "2099-06-01", "2099-06-12", "planned"),
        )
        .unwrap();
        fs::write(
            backlog_dir.join(story_file),
            "---\nid: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story: Add read-only CLI for sprint and backlog inspection\n",
        )
        .unwrap();

        let story = read_story_file(backlog_dir.join(story_file), temp_root.path()).unwrap();
        let validation = validate_repository(temp_root.path()).unwrap();

        assert_eq!(story.sprint_name.as_deref(), Some("S001.foundation"));
        assert!(validation.issues.is_empty());
    }

    #[test]
    fn move_story_to_status_updates_single_story_and_roster() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_git_config(temp_root.path(), "Test User", "test@example.com");
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "planned",
        );
        let story_path = write_story_with_task_file(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Old Owner <old@example.com>\nstory_points: 8\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = move_story_to_status(temp_root.path(), "US-F1-053", "in-progress").unwrap();

        assert_eq!(result.to_status, "in-progress");
        let moved_story = fs::read_to_string(&story_path).unwrap();
        assert!(moved_story.contains("status: in-progress"));
        assert!(moved_story.contains("assignee: Test User <test@example.com>"));
        assert!(moved_story.contains("work_started: 20"));
        let sprint_markdown =
            fs::read_to_string(temp_root.path().join("delivery/sprints/S001.foundation.md"))
                .unwrap();
        assert!(sprint_markdown.contains("- in-progress: US-F1-053"));
        assert_eq!(
            result.task_path,
            Some(relative_path(
                temp_root.path(),
                &story_path.with_extension("tasks.md")
            ))
        );
    }

    #[test]
    fn move_story_to_status_updates_story_in_place_without_creating_status_folders() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "active",
        );
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = move_story_to_status(temp_root.path(), "US-F1-053", "ready-for-qa").unwrap();

        assert_eq!(result.to_status, "ready-for-qa");
        assert_eq!(temp_root.path().join(&result.story_path), story_path);
        let moved_story = fs::read_to_string(&story_path).unwrap();
        assert!(moved_story.contains("status: ready-for-qa"));
    }

    #[test]
    fn move_story_to_in_progress_refreshes_assignee_when_already_in_progress() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_git_config(temp_root.path(), "Test User", "test@example.com");
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "active",
        );
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Old Owner <old@example.com>\nstory_points: 8\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = move_story_to_status(temp_root.path(), "US-F1-053", "in-progress").unwrap();

        assert_eq!(result.to_status, "in-progress");
        assert_eq!(temp_root.path().join(result.story_path), story_path);
        let backlog_story = fs::read_to_string(&story_path).unwrap();
        assert!(backlog_story.contains("assignee: Test User <test@example.com>"));
    }

    #[test]
    fn move_story_to_in_progress_uses_assignee_override() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "planned",
        );
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = move_story_to_status_with_assignee(
            temp_root.path(),
            "US-F1-053",
            "in-progress",
            Some("Override User <override@example.com>"),
        )
        .unwrap();

        assert_eq!(temp_root.path().join(result.story_path), story_path);
        let backlog_story = fs::read_to_string(&story_path).unwrap();
        assert!(backlog_story.contains("assignee: Override User <override@example.com>"));
    }

    #[test]
    fn move_story_rejects_invalid_assignee_override_before_moving_files() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "planned",
        );
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let err = move_story_to_status_with_assignee(
            temp_root.path(),
            "US-F1-053",
            "in-progress",
            Some("Invalid User"),
        )
        .unwrap_err();

        assert!(err.to_string().contains("Name <email>"));
        assert!(story_path.exists());
        assert!(
            !temp_root
                .path()
                .join("delivery/sprints/S001.foundation/02.in-progress")
                .exists()
        );
    }

    #[test]
    fn move_story_to_done_refreshes_existing_work_done() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "active",
        );
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started: 2026-05-28T14:05:54+0200\nwork_done: 1999-01-01T00:00:00+0100\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = move_story_to_status(temp_root.path(), "US-F1-053", "done").unwrap();

        assert_eq!(result.to_status, "done");
        assert_eq!(temp_root.path().join(result.story_path), story_path);
        let moved_story = fs::read_to_string(&story_path).unwrap();
        let backlog_story = moved_story.clone();
        assert!(moved_story.contains("status: done"));
        assert!(!moved_story.contains("work_done: 1999-01-01T00:00:00+0100"));
        assert!(!backlog_story.contains("work_done: 1999-01-01T00:00:00+0100"));
        assert!(moved_story.contains("work_done: 20"));
        assert!(backlog_story.contains("work_done: 20"));
    }

    #[test]
    fn plan_story_into_sprint_updates_story_in_place() {
        let temp_root = tempdir().unwrap();
        write_git_config(temp_root.path(), "Test User", "test@example.com");
        init_temp_repo(temp_root.path());

        write_sprint_file(
            temp_root.path(),
            "S001.planning",
            "planning",
            "2999-01-04",
            "2999-01-15",
            "planned",
        );

        let backlog_dir = temp_root
            .path()
            .join("delivery/backlog/phase-2-core-logic/01.passage-ingestion");
        fs::create_dir_all(&backlog_dir).unwrap();
        let backlog_story = backlog_dir.join("US-F2-001-ingest-passage-events.md");
        fs::write(
            &backlog_story,
            "---\nid: US-F2-001\ntype: user-story\nstatus: todo\nepic: EP-F2-01\nsprint:\nstory_points: 8\nactivated:\ncreated: 2026-05-20\nupdated: 2026-05-20\n---\n\n# User Story: Ingest passage events\n",
        )
        .unwrap();

        let result =
            plan_story_into_sprint(temp_root.path(), "US-F2-001", "S001.planning").unwrap();

        assert_eq!(result.story_id, "US-F2-001");
        assert_eq!(result.sprint_name, "S001.planning");

        let story = read_story_file(&backlog_story, temp_root.path()).unwrap();
        assert_eq!(
            story.frontmatter.get("status").map(String::as_str),
            Some("todo")
        );
        assert_eq!(
            story.frontmatter.get("sprint").map(String::as_str),
            Some("S001.planning")
        );
        assert!(
            story
                .frontmatter
                .get("activated")
                .map(|value| !value.is_empty())
                .unwrap_or(false)
        );
        let sprint_markdown =
            fs::read_to_string(temp_root.path().join("delivery/sprints/S001.planning.md")).unwrap();
        assert!(sprint_markdown.contains("- todo: US-F2-001"));
    }

    #[test]
    fn plan_story_into_sprint_rejects_unknown_sprint() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let backlog_dir = temp_root.path().join("doc/backlog/phase-2-core-logic/01.x");
        fs::create_dir_all(&backlog_dir).unwrap();
        fs::write(
            backlog_dir.join("US-F2-009-x.md"),
            "---\nid: US-F2-009\ntype: user-story\nstatus: todo\nepic: EP-F2-01\nsprint:\nstory_points: 3\n---\n\n# x\n",
        )
        .unwrap();

        let err = plan_story_into_sprint(temp_root.path(), "US-F2-009", "S404.nope").unwrap_err();
        assert!(err.to_string().contains("S404.nope"));
    }

    #[test]
    fn task_mutations_update_sibling_task_file_only() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2099-06-01",
            "2099-06-12",
            "active",
        );
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );
        fs::write(
            story_path.with_extension("tasks.md"),
            "# Tasks for US-F1-053\n\nParent User Story: US-F1-053\nSprint: S001.foundation\n\n---\n",
        ).unwrap();

        let add_result = add_task_to_story(
            temp_root.path(),
            "US-F1-053",
            "Add new task",
            "todo",
            &["cli".to_string(), "write".to_string()],
            "Add command coverage.",
        )
        .unwrap();
        let task_markdown =
            fs::read_to_string(temp_root.path().join(add_result.task_file_path.clone())).unwrap();
        assert!(task_markdown.contains("TASK-US-F1-053-001"));
        assert!(task_markdown.contains("Add new task"));

        update_task_in_story(
            temp_root.path(),
            "US-F1-053",
            "TASK-US-F1-053-001",
            Some("done"),
            None,
            None,
            Some("Completed command coverage."),
        )
        .unwrap();
        let updated_markdown =
            fs::read_to_string(temp_root.path().join(add_result.task_file_path)).unwrap();
        assert!(updated_markdown.contains("Status: Done"));
        assert!(updated_markdown.contains("Completed command coverage."));
    }

    #[test]
    fn task_update_preserves_other_task_headings() {
        let markdown = "# Tasks for US-F1-053\n\n---\n\n## TASK-US-F1-053-001 - First task\n\nStatus: To Do\nTags: docs\n\nDescription:\nFirst.\n\n---\n\n## TASK-US-F1-053-002 - Second task\n\nStatus: To Do\nTags: cli\n\nDescription:\nSecond.\n\n---\n\n## TASK-US-F1-053-003 - Third task\n\nStatus: To Do\nTags: tests\n\nDescription:\nThird.\n";

        let updated = rewrite_task_markdown(
            markdown,
            "TASK-US-F1-053-002",
            Some("done"),
            None,
            None,
            None,
        )
        .unwrap();

        assert!(updated.contains("## TASK-US-F1-053-001 - First task"));
        assert!(updated.contains("## TASK-US-F1-053-002 - Second task"));
        assert!(updated.contains("## TASK-US-F1-053-003 - Third task"));
        assert!(updated.contains("Status: Done"));
    }

    #[test]
    fn rollover_moves_unfinished_stories_and_updates_closed_summary() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let backlog_dir = temp_root
            .path()
            .join("delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling");

        fs::create_dir_all(&backlog_dir).unwrap();
        let sprint_file = temp_root.path().join("delivery/sprints/S001.foundation.md");
        fs::create_dir_all(sprint_file.parent().unwrap()).unwrap();
        fs::write(
            &sprint_file,
            format!(
                "{}\n## End-Of-Sprint Summary\n\nSprint still active.\n\n## Expected Carry-Over / Unfinished Stories\n\nNot determined yet.\n",
                sprint_readme("S001", "foundation", "2099-06-01", "2099-06-12", "active")
            ),
        ).unwrap();
        fs::write(
            backlog_dir.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md"),
            "---\nid: US-F1-052\ntype: user-story\nstatus: done\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started: 2026-05-28T16:30:54+0200\nwork_done: 2026-05-28T22:06:38+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T22:06:38+0200\n---\n# User Story: Add read-only CLI for sprint and backlog inspection\n",
        ).unwrap();
        fs::write(
            backlog_dir.join("US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md"),
            "---\nid: US-F1-053\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started: 2026-05-28T22:35:00+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T22:35:00+0200\n---\n# User Story: Add CLI support for status moves and sprint rollover\n",
        ).unwrap();

        let next_start = NaiveDate::from_ymd_opt(2099, 6, 15).unwrap();
        let next_end = NaiveDate::from_ymd_opt(2099, 6, 26).unwrap();
        let next_input = CreateSprintInput {
            number: 2,
            start_date: next_start,
            end_date: next_end,
            headline: "next-sprint".to_string(),
        };

        let result =
            rollover_sprint(temp_root.path(), "S001.foundation", Some(&next_input)).unwrap();

        assert!(result.created_next_sprint);
        assert_eq!(result.completed_story_ids, vec!["US-F1-052".to_string()]);
        assert_eq!(result.carried_story_ids, vec!["US-F1-053".to_string()]);
        let carried_story = fs::read_to_string(
            backlog_dir.join("US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md"),
        )
        .unwrap();
        assert!(carried_story.contains("sprint: S002.next-sprint"));
        let closed_summary = fs::read_to_string(&sprint_file).unwrap();
        assert!(closed_summary.contains("Completed stories in `S001.foundation`: US-F1-052."));
        assert!(closed_summary.contains("Moved to `S002.next-sprint`: US-F1-053."));
    }
}
