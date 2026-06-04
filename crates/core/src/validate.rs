use crate::config::*;
use crate::constants::*;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::sprint::*;
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
