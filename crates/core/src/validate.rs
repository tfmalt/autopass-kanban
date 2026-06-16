use crate::config::*;
use crate::constants::*;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::sprint::*;
use crate::story::*;
use crate::util::*;

pub fn validate_story(story: &Story) -> Vec<ValidationIssue> {
    let config = story
        .file_path
        .parent()
        .and_then(|parent| load_kanban_config(parent).ok());
    let mut issues = Vec::new();

    for field_name in REQUIRED_STORY_FIELDS {
        if !story.frontmatter_keys.contains(field_name) {
            add_issue(
                story,
                &mut issues,
                format!("missing-field:{field_name}"),
                format!("Missing required frontmatter field \"{field_name}\"."),
            );
        }
    }

    if story.frontmatter_keys.contains("status") {
        let status = story
            .frontmatter
            .get("status")
            .map(String::as_str)
            .unwrap_or_default();
        if !CANONICAL_STORY_STATUSES.contains(&status) {
            add_issue(
                story,
                &mut issues,
                "non-canonical-status",
                format!(
                    "Story status \"{status}\" is not part of the canonical workflow vocabulary."
                ),
            );
        }
    }

    if story.frontmatter_keys.contains("story_points") {
        let story_points = story
            .frontmatter
            .get("story_points")
            .map(String::as_str)
            .unwrap_or_default();
        let accepted_values = config
            .as_ref()
            .map(|config| config.story_points.accepted_values())
            .unwrap_or_else(|| {
                ["2", "3", "5", "8", "13", "XS", "S", "M", "L", "XL"]
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            });
        if !accepted_values.contains(story_points) {
            add_issue(
                story,
                &mut issues,
                "invalid-story-points",
                format!(
                    "story_points must be one of {}.",
                    accepted_values.into_iter().collect::<Vec<_>>().join(", ")
                ),
            );
        }
    }

    validate_optional_priority_field(
        &story.relative_path,
        &story.frontmatter,
        &story.frontmatter_keys,
        &mut issues,
    );

    validate_timestamp_field(story, &mut issues, "created", false, true);
    validate_timestamp_field(story, &mut issues, "updated", false, true);
    validate_timestamp_field(story, &mut issues, "work_started", true, false);
    validate_timestamp_field(story, &mut issues, "work_done", true, false);

    if story
        .frontmatter
        .get("sprint")
        .is_some_and(|sprint| !sprint.trim().is_empty() && sprint.as_str() != "~")
    {
        validate_timestamp_field(story, &mut issues, "activated", true, false);
    }

    if story.frontmatter.get("status").map(String::as_str) == Some("in-progress")
        && story
            .frontmatter
            .get("work_started")
            .map(String::as_str)
            .unwrap_or_default()
            .is_empty()
    {
        add_issue(
            story,
            &mut issues,
            "missing-work-started",
            "Stories in progress must have work_started set.".to_string(),
        );
    }

    if assignee_required(story) && !assignee_is_set(story) {
        add_issue(
            story,
            &mut issues,
            "missing-field:assignee",
            "Stories outside draft/todo must have assignee set.".to_string(),
        );
    }

    if story.frontmatter.get("status").map(String::as_str) == Some("done")
        && story
            .frontmatter
            .get("work_done")
            .map(String::as_str)
            .unwrap_or_default()
            .is_empty()
    {
        add_issue(
            story,
            &mut issues,
            "missing-work-done",
            "Done stories must have work_done set.".to_string(),
        );
    }

    issues
}

pub fn validate_repository(repo_root: impl AsRef<Path>) -> Result<ValidationReport> {
    let repository = read_repository(repo_root)?;
    let mut issues = Vec::new();
    let config = load_kanban_config(&repository.repo_root)?;

    issues.extend(validate_sprint_readmes(&config)?);

    for epic_file in collect_epic_files(&repository.repo_root)? {
        let epic = read_epic_file(epic_file, &repository.repo_root)?;
        issues.extend(validate_epic(&epic));
    }

    for story in &repository.stories {
        issues.extend(validate_story(story));
        if let Some(task_file) = &story.task_file
            && !task_file.exists
            && story.frontmatter.get("status").map(String::as_str) != Some("todo")
        {
            add_issue(
                story,
                &mut issues,
                "missing-task-file",
                format!(
                    "Referenced task file does not exist: {}",
                    task_file.relative_path.display()
                ),
            );
        }
    }

    Ok(ValidationReport {
        repo_root: repository.repo_root,
        stories: repository.stories,
        issues,
    })
}

pub(crate) fn validate_sprint_readmes(config: &KanbanConfig) -> Result<Vec<ValidationIssue>> {
    let repo_root = &config.repo_root;
    let sprints_root = config.sprints_path();
    let mut issues = Vec::new();

    for entry in fs::read_dir(&sprints_root)
        .with_context(|| format!("read sprint root {}", sprints_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative_file = relative_path(repo_root, &path);
        let Some(file_name) = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
        else {
            continue;
        };
        if file_name == "README.md" {
            continue;
        }

        let parsed_file = parse_sprint_file_name(&file_name);
        if parsed_file.is_none() {
            issues.push(ValidationIssue {
                file_path: relative_file.clone(),
                rule: "invalid-sprint-folder-name".to_string(),
                message: "Sprint file name must use `<Snnn>.<headline-slug>.md`.".to_string(),
            });
        }

        let markdown = fs::read_to_string(&path)
            .with_context(|| format!("read sprint file {}", path.display()))?;
        let parsed = parse_frontmatter(&markdown);
        for field_name in REQUIRED_SPRINT_README_FIELDS {
            if !parsed.frontmatter_keys.contains(field_name) {
                issues.push(ValidationIssue {
                    file_path: relative_file.clone(),
                    rule: format!("missing-sprint-readme-field:{field_name}"),
                    message: format!(
                        "Missing required sprint file frontmatter field \"{field_name}\"."
                    ),
                });
            }
        }

        if let Some((sprint_id, headline)) = parsed_file {
            if parsed.frontmatter.get("sprint").map(String::as_str) != Some(sprint_id.as_str()) {
                issues.push(ValidationIssue {
                    file_path: relative_file.clone(),
                    rule: "sprint-readme-folder-mismatch:sprint".to_string(),
                    message: format!(
                        "Sprint file field \"sprint\" must match file sprint id \"{sprint_id}\"."
                    ),
                });
            }
            if parsed.frontmatter.get("headline").map(String::as_str) != Some(headline.as_str()) {
                issues.push(ValidationIssue {
                    file_path: relative_file.clone(),
                    rule: "sprint-readme-folder-mismatch:headline".to_string(),
                    message: format!(
                        "Sprint file field \"headline\" must match file headline \"{headline}\"."
                    ),
                });
            }
        }

        for (field_name, table_label) in [
            ("sprint", "Sprint Name"),
            ("start_date", "Start Date"),
            ("end_date", "End Date"),
            ("status", "Sprint Status"),
            ("wip_limit", "WIP Limit"),
        ] {
            if parsed.frontmatter_keys.contains(field_name)
                && readme_table_value(&markdown, table_label).is_some()
            {
                issues.push(ValidationIssue {
                    file_path: relative_file.clone(),
                    rule: "duplicated-sprint-metadata".to_string(),
                    message: format!(
                        "Sprint metadata \"{field_name}\" is duplicated in the README body; frontmatter is canonical."
                    ),
                });
            }
        }

        for field_name in ["start_date", "end_date"] {
            if let Some(value) = parsed.frontmatter.get(field_name)
                && parse_markdown_date(value).is_none()
            {
                issues.push(ValidationIssue {
                    file_path: relative_file.clone(),
                    rule: format!("invalid-sprint-readme-date:{field_name}"),
                    message: format!("Sprint README field \"{field_name}\" must use YYYY-MM-DD."),
                });
            }
        }

        if let Some(status) = parsed.frontmatter.get("status")
            && !SPRINT_STATUSES.contains(&status.as_str())
        {
            issues.push(ValidationIssue {
                file_path: relative_file.clone(),
                rule: "invalid-sprint-readme-status".to_string(),
                message:
                    "Sprint file field \"status\" must be planned, active, closed, or cancelled."
                        .to_string(),
            });
        }
    }

    Ok(issues)
}

fn validate_epic(epic: &Epic) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    validate_optional_priority_field(
        &epic.relative_path,
        &epic.frontmatter,
        &epic.frontmatter_keys,
        &mut issues,
    );
    issues
}

fn validate_optional_priority_field(
    file_path: &Path,
    frontmatter: &BTreeMap<String, String>,
    frontmatter_keys: &BTreeSet<String>,
    issues: &mut Vec<ValidationIssue>,
) {
    if !frontmatter_keys.contains("priority") {
        return;
    }

    let value = frontmatter
        .get("priority")
        .map(String::as_str)
        .unwrap_or_default();
    if validate_non_negative_integer_frontmatter("priority", value).is_err() {
        issues.push(ValidationIssue {
            file_path: file_path.to_path_buf(),
            rule: "invalid-priority".to_string(),
            message: "Frontmatter field \"priority\" must be a non-negative integer.".to_string(),
        });
    }
}

pub(crate) fn validate_timestamp_field(
    story: &Story,
    issues: &mut Vec<ValidationIssue>,
    field_name: &str,
    allow_empty: bool,
    allow_date_only: bool,
) {
    let value = story
        .frontmatter
        .get(field_name)
        .map(String::as_str)
        .unwrap_or_default();
    let status = story.frontmatter.get("status").map(String::as_str);
    let status_allows_null_work_started =
        field_name == "work_started" && value == "null" && matches!(status, Some("draft" | "todo"));
    let status_allows_null_work_done =
        field_name == "work_done" && value == "null" && !matches!(status, Some("done"));
    if status_allows_null_work_started || status_allows_null_work_done {
        return;
    }
    if allow_empty && value.is_empty() {
        return;
    }

    let timestamp_pattern = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{4}$")
        .expect("valid timestamp regex");
    let date_pattern = Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("valid date regex");
    if !(timestamp_pattern.is_match(value) || allow_date_only && date_pattern.is_match(value)) {
        let expected_format = if allow_date_only {
            "local ISO 8601 with numeric timezone offset or YYYY-MM-DD"
        } else {
            "local ISO 8601 with numeric timezone offset"
        };
        add_issue(
            story,
            issues,
            format!("invalid-timestamp:{field_name}"),
            format!("Frontmatter field \"{field_name}\" must use {expected_format}."),
        );
    }
}

pub(crate) fn assignee_required(story: &Story) -> bool {
    !matches!(
        story.frontmatter.get("status").map(String::as_str),
        Some("draft" | "todo")
    )
}

pub(crate) fn assignee_is_set(story: &Story) -> bool {
    story
        .frontmatter
        .get("assignee")
        .is_some_and(|value| !value.trim().is_empty())
}

pub(crate) fn add_issue(
    story: &Story,
    issues: &mut Vec<ValidationIssue>,
    rule: impl Into<String>,
    message: String,
) {
    issues.push(ValidationIssue {
        file_path: story.relative_path.clone(),
        rule: rule.into(),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use tempfile::tempdir;

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
    fn validate_story_accepts_backlog_status_as_ready_synonym() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root.path().join(
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-050-backlog-status.md",
        );

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-050\ntype: user-story\nstatus: backlog\nepic: EP-F1-06\nsprint: ~\nassignee: Test User <test@example.com>\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(!rules.contains(&"non-canonical-status"));
    }

    #[test]
    fn validate_story_allows_null_work_started_for_draft_and_todo() {
        for status in ["draft", "todo"] {
            let temp_root = tempdir().unwrap();
            init_temp_repo(temp_root.path());
            let story_path = temp_root
                .path()
                .join(format!("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-05{}-null-work-started.md", if status == "draft" { 0 } else { 1 }));

            fs::create_dir_all(story_path.parent().unwrap()).unwrap();
            fs::write(
                &story_path,
                format!("---\nid: US-F1-050\ntype: user-story\nstatus: {status}\nepic: EP-F1-06\nsprint: ~\nstory_points: 3\nwork_started: null\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n"),
            )
            .unwrap();

            let story = read_story_file(story_path, temp_root.path()).unwrap();
            let issues = validate_story(&story);
            let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

            assert!(!rules.contains(&"invalid-timestamp:work_started"));
        }
    }

    #[test]
    fn validate_story_allows_null_work_done_unless_done() {
        for status in [
            "draft",
            "todo",
            "in-progress",
            "ready-for-qa",
            "blocked",
            "dropped",
        ] {
            let temp_root = tempdir().unwrap();
            init_temp_repo(temp_root.path());
            let story_path = temp_root.path().join(format!(
                "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-060-null-work-done-{status}.md"
            ));

            fs::create_dir_all(story_path.parent().unwrap()).unwrap();
            fs::write(
                &story_path,
                format!("---\nid: US-F1-060\ntype: user-story\nstatus: {status}\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 3\nwork_started:\nwork_done: null\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n"),
            )
            .unwrap();

            let story = read_story_file(story_path, temp_root.path()).unwrap();
            let issues = validate_story(&story);
            let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

            assert!(!rules.contains(&"invalid-timestamp:work_done"));
        }
    }

    #[test]
    fn validate_story_rejects_null_work_done_when_done() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root.path().join(
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-061-null-work-done-when-done.md",
        );

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-061\ntype: user-story\nstatus: done\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 3\nwork_started: 2026-05-28T14:05:54+0200\nwork_done: null\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"invalid-timestamp:work_done"));
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
    fn validate_story_rejects_invalid_priority_value() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root.path().join(
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-062-invalid-priority.md",
        );

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-062\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 3\npriority: -1\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"invalid-priority"));
    }

    #[test]
    fn validate_repository_rejects_invalid_epic_priority_value() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let epic_path = temp_root.path().join(
            "delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-062-invalid-priority.md",
        );

        fs::create_dir_all(epic_path.parent().unwrap()).unwrap();
        fs::write(
            &epic_path,
            "---\nid: EP-F1-062\ntype: epic\nstatus: draft\nphase: 1\npriority: -1\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# Epic: Invalid priority\n",
        )
        .unwrap();

        let validation = validate_repository(temp_root.path()).unwrap();
        assert!(validation
            .issues
            .iter()
            .any(|issue| {
                issue.rule == "invalid-priority"
                    && issue.file_path.as_path()
                        == Path::new(
                            "delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-062-invalid-priority.md",
                        )
            }));
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
}
