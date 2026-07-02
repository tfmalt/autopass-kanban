use std::collections::BTreeMap;

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

/// Returns whether a given story frontmatter field is required, given the active feature flags.
fn field_required_when_features(field_name: &str, features: &FeaturesConfig) -> bool {
    match field_name {
        "sprint" => features.sprints,
        "epic" => features.epics,
        _ => true,
    }
}

pub fn validate_story(story: &Story, features: &FeaturesConfig) -> Vec<ValidationIssue> {
    validate_story_with_config(story, features, None)
}

/// Validate a story using an already-loaded [`KanbanConfig`] (US-026).
///
/// Unlike [`validate_story`], this does not call `load_kanban_config(parent)`
/// to rediscover the config — the caller passes it in. When `config` is
/// `None`, default story-point accepted values are used (for backward
/// compatibility with tests that don't construct a full config).
pub fn validate_story_with_config(
    story: &Story,
    features: &FeaturesConfig,
    config: Option<&KanbanConfig>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    for field_name in REQUIRED_STORY_FIELDS {
        if !field_required_when_features(field_name, features) {
            continue;
        }
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
            .map(|c| c.story_points.accepted_values())
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

    if story.frontmatter_keys.contains("task_file")
        && let Some(value) = story.frontmatter.get("task_file")
        && !value.trim().is_empty()
        && validate_task_file_frontmatter_value(value).is_err()
    {
        add_issue(
            story,
            &mut issues,
            "invalid-task-file-path",
            format!(
                "task_file must be a sibling file name without `..`, path separators, or absolute paths; got {value:?}."
            ),
        );
    }

    validate_timestamp_field(story, &mut issues, "created", false, true);
    validate_timestamp_field(story, &mut issues, "updated", false, true);
    validate_timestamp_field(story, &mut issues, "work_started", true, false);
    validate_timestamp_field(story, &mut issues, "work_done", true, false);

    if features.sprints
        && story
            .frontmatter
            .get("sprint")
            .is_some_and(|sprint| !sprint.trim().is_empty() && sprint.as_str() != "~")
    {
        if let Some(sprint) = story.frontmatter.get("sprint")
            && validate_story_sprint_frontmatter(sprint).is_err()
        {
            add_issue(
                story,
                &mut issues,
                "invalid-sprint",
                format!(
                    "Story sprint \"{sprint}\" must be empty, ~, or use <Snnn>.<headline-slug>."
                ),
            );
        }
        validate_timestamp_field(story, &mut issues, "activated", true, false);

        if story.frontmatter.get("status").map(String::as_str) == Some("planned") {
            add_issue(
                story,
                &mut issues,
                "planned-status-in-sprint",
                "Stories assigned to a sprint must use status `todo` instead of `planned`."
                    .to_string(),
            );
        }
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
            "Stories outside draft/ready/planned/todo must have assignee set.".to_string(),
        );
    }

    if story
        .frontmatter
        .get("status")
        .map(String::as_str)
        .is_some_and(|status| matches!(status, "done" | "dropped"))
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
            "Done and dropped stories must have work_done set.".to_string(),
        );
    }

    // US-026: detect story files whose canonicalized path is outside the
    // backlog root (defense-in-depth against symlink planting or moved files).
    if let Some(config) = config {
        let backlog_root = config.backlog_path();
        let canonical_backlog = backlog_root
            .canonicalize()
            .unwrap_or_else(|_| backlog_root.clone());
        if let Ok(canonical_story) = story.file_path.canonicalize()
            && !canonical_story.starts_with(&canonical_backlog)
        {
            add_issue(
                story,
                &mut issues,
                "out-of-tree-story-path",
                format!(
                    "Story file canonical path {} is outside the backlog root {}.",
                    canonical_story.display(),
                    canonical_backlog.display()
                ),
            );
        }
    }

    issues
}

pub fn validate_repository(repo_root: impl AsRef<Path>) -> Result<ValidationReport> {
    let repository = read_repository(repo_root)?;
    let mut issues = Vec::new();
    let config = load_kanban_config(&repository.repo_root)?;

    let features = config.features();
    if features.sprints {
        issues.extend(validate_sprint_readmes(&config)?);
    }

    if features.epics {
        for epic_file in collect_epic_files(&repository.repo_root)? {
            let epic = read_epic_file(epic_file, &repository.repo_root)?;
            issues.extend(validate_epic(&epic));
        }
    }

    for story in &repository.stories {
        issues.extend(validate_story_with_config(story, &features, Some(&config)));
        if let Some(task_file) = &story.task_file
            && !task_file.exists
            && !matches!(
                story.frontmatter.get("status").map(String::as_str),
                Some("planned" | "todo")
            )
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

    // US-026: detect duplicate story IDs across the backlog tree.
    let mut ids_by_value: BTreeMap<&str, Vec<&Story>> = BTreeMap::new();
    for story in &repository.stories {
        let id = story
            .frontmatter
            .get("id")
            .map(String::as_str)
            .unwrap_or("");
        if !id.is_empty() {
            ids_by_value.entry(id).or_default().push(story);
        }
    }
    for (id, stories) in &ids_by_value {
        if stories.len() > 1 {
            for story in stories {
                add_issue(
                    story,
                    &mut issues,
                    "duplicate-story-id",
                    format!(
                        "Story ID \"{id}\" appears in {} files (also at {}).",
                        stories.len(),
                        stories
                            .iter()
                            .filter(|s| s.relative_path != story.relative_path)
                            .map(|s| s.relative_path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
            }
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
    validate_optional_markdown_date_field(epic, &mut issues, "planned_start");
    validate_optional_markdown_date_field(epic, &mut issues, "planned_end");
    validate_optional_epic_timestamp_field(epic, &mut issues, "work_started");
    validate_optional_epic_timestamp_field(epic, &mut issues, "work_done");
    validate_epic_field_order(epic, &mut issues, "planned_start", "planned_end");
    validate_epic_field_order(epic, &mut issues, "work_started", "work_done");
    issues
}

fn validate_optional_markdown_date_field(
    epic: &Epic,
    issues: &mut Vec<ValidationIssue>,
    field_name: &str,
) {
    if !epic.frontmatter_keys.contains(field_name) {
        return;
    }
    let value = epic
        .frontmatter
        .get(field_name)
        .map(String::as_str)
        .unwrap_or_default();
    if validate_markdown_date_frontmatter(field_name, value).is_err() {
        issues.push(ValidationIssue {
            file_path: epic.relative_path.clone(),
            rule: format!("invalid-epic-date:{field_name}"),
            message: format!("Epic frontmatter field \"{field_name}\" must use YYYY-MM-DD."),
        });
    }
}

fn validate_optional_epic_timestamp_field(
    epic: &Epic,
    issues: &mut Vec<ValidationIssue>,
    field_name: &str,
) {
    if !epic.frontmatter_keys.contains(field_name) {
        return;
    }
    let value = epic
        .frontmatter
        .get(field_name)
        .map(String::as_str)
        .unwrap_or_default();
    if validate_local_timestamp_frontmatter(field_name, value).is_err() {
        issues.push(ValidationIssue {
            file_path: epic.relative_path.clone(),
            rule: format!("invalid-epic-timestamp:{field_name}"),
            message: format!(
                "Epic frontmatter field \"{field_name}\" must use local ISO 8601 with numeric timezone offset."
            ),
        });
    }
}

fn validate_epic_field_order(
    epic: &Epic,
    issues: &mut Vec<ValidationIssue>,
    start_field: &str,
    end_field: &str,
) {
    let Some(start) = epic.frontmatter.get(start_field).map(String::as_str) else {
        return;
    };
    let Some(end) = epic.frontmatter.get(end_field).map(String::as_str) else {
        return;
    };
    if start.trim().is_empty()
        || end.trim().is_empty()
        || matches!(start, "~" | "null")
        || matches!(end, "~" | "null")
    {
        return;
    }
    if start > end {
        issues.push(ValidationIssue {
            file_path: epic.relative_path.clone(),
            rule: format!("epic-date-order:{start_field}:{end_field}"),
            message: format!(
                "Epic frontmatter field \"{start_field}\" must be earlier than or equal to \"{end_field}\"."
            ),
        });
    }
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
    let status_allows_null_work_started = field_name == "work_started"
        && value == "null"
        && matches!(status, Some("draft" | "planned" | "todo"));
    let status_allows_null_work_done =
        field_name == "work_done" && value == "null" && !matches!(status, Some("done" | "dropped"));
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

pub(crate) fn validate_markdown_date_frontmatter(field_name: &str, value: &str) -> Result<()> {
    let value = value.trim();
    if value.is_empty() || value == "~" {
        return Ok(());
    }
    if parse_markdown_date(value).is_none() {
        bail!("Frontmatter field \"{field_name}\" must use YYYY-MM-DD.");
    }
    Ok(())
}

pub(crate) fn validate_local_timestamp_frontmatter(field_name: &str, value: &str) -> Result<()> {
    let value = value.trim();
    if value.is_empty() || matches!(value, "~" | "null") {
        return Ok(());
    }
    let timestamp_pattern = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{4}$")
        .expect("valid timestamp regex");
    if !timestamp_pattern.is_match(value) {
        bail!(
            "Frontmatter field \"{field_name}\" must use local ISO 8601 with numeric timezone offset."
        );
    }
    Ok(())
}

pub(crate) fn assignee_required(story: &Story) -> bool {
    !matches!(
        story.frontmatter.get("status").map(String::as_str),
        Some("draft" | "ready" | "planned" | "todo")
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
        assert!(validate_story(&story, &FeaturesConfig::default()).is_empty());
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
        let issues = validate_story(&story, &FeaturesConfig::default());
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
        let issues = validate_story(&story, &FeaturesConfig::default());
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(!rules.contains(&"non-canonical-status"));
    }

    #[test]
    fn validate_story_allows_null_work_started_for_draft_planned_and_todo() {
        for status in ["draft", "planned", "todo"] {
            let temp_root = tempdir().unwrap();
            init_temp_repo(temp_root.path());
            let story_path = temp_root
                .path()
                .join(format!("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-05-{status}-null-work-started.md"));

            fs::create_dir_all(story_path.parent().unwrap()).unwrap();
            fs::write(
                &story_path,
                format!("---\nid: US-F1-050\ntype: user-story\nstatus: {status}\nepic: EP-F1-06\nsprint: ~\nstory_points: 3\nwork_started: null\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n"),
            )
            .unwrap();

            let story = read_story_file(story_path, temp_root.path()).unwrap();
            let issues = validate_story(&story, &FeaturesConfig::default());
            let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

            assert!(!rules.contains(&"invalid-timestamp:work_started"));
        }
    }

    #[test]
    fn validate_story_allows_null_work_done_unless_done_or_dropped() {
        for status in ["draft", "todo", "in-progress", "ready-for-qa", "blocked"] {
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
            let issues = validate_story(&story, &FeaturesConfig::default());
            let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

            assert!(!rules.contains(&"invalid-timestamp:work_done"));
        }
    }

    #[test]
    fn validate_story_rejects_null_work_done_when_terminal() {
        for status in ["done", "dropped"] {
            let temp_root = tempdir().unwrap();
            init_temp_repo(temp_root.path());
            let story_path = temp_root.path().join(format!(
                "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-061-null-work-done-when-{status}.md"
            ));

            fs::create_dir_all(story_path.parent().unwrap()).unwrap();
            fs::write(
                &story_path,
                format!("---\nid: US-F1-061\ntype: user-story\nstatus: {status}\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 3\nwork_started: 2026-05-28T14:05:54+0200\nwork_done: null\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n"),
            )
            .unwrap();

            let story = read_story_file(story_path, temp_root.path()).unwrap();
            let issues = validate_story(&story, &FeaturesConfig::default());
            let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

            assert!(rules.contains(&"invalid-timestamp:work_done"));
        }
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
        let issues = validate_story(&story, &FeaturesConfig::default());
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(!rules.contains(&"missing-field:assignee"));
    }

    #[test]
    fn validate_story_rejects_planned_status_when_story_is_in_sprint() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-planned-in-sprint.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-052\ntype: user-story\nstatus: planned\nepic: EP-F1-06\nsprint: S001.foundation\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story, &FeaturesConfig::default());
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"planned-status-in-sprint"));
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
        let issues = validate_story(&story, &FeaturesConfig::default());
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
        let issues = validate_story(&story, &FeaturesConfig::default());
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
        let issues = validate_story(&story, &FeaturesConfig::default());
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"invalid-priority"));
    }

    #[test]
    fn validate_story_rejects_invalid_sprint_value() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root.path().join(
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-063-invalid-sprint.md",
        );

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-063\ntype: user-story\nstatus: ready\nepic: EP-F1-06\nsprint: /Users/tm\nassignee: Test User <test@example.com>\nstory_points: 3\nwork_started:\nwork_done:\nactivated: 2026-05-28T14:05:54+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story, &FeaturesConfig::default());
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"invalid-sprint"));
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
    fn validate_repository_rejects_invalid_epic_lifecycle_values() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let epic_path = temp_root.path().join(
            "delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-063-invalid-lifecycle.md",
        );

        fs::create_dir_all(epic_path.parent().unwrap()).unwrap();
        fs::write(
            &epic_path,
            "---\nid: EP-F1-063\ntype: epic\nstatus: done\nphase: 1\nplanned_start: 2026/06/15\nplanned_end: 2026-06-10\nwork_started: 2026-06-12T09:00:00+0200\nwork_done: 2026-06-11T09:00:00+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# Epic: Invalid lifecycle\n",
        )
        .unwrap();

        let validation = validate_repository(temp_root.path()).unwrap();
        let rules: Vec<&str> = validation
            .issues
            .iter()
            .map(|issue| issue.rule.as_str())
            .collect();
        assert!(rules.contains(&"invalid-epic-date:planned_start"));
        assert!(rules.contains(&"epic-date-order:work_started:work_done"));
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

    #[test]
    fn validate_story_skips_sprint_field_when_sprints_feature_disabled() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "features.sprints", "false").unwrap();
        set_config_value(temp_root.path(), "paths.sprints", "").unwrap();

        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-090-no-sprint.md");
        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-090\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let config = load_kanban_config(temp_root.path()).unwrap();
        let features = config.features();
        assert!(!features.sprints);
        assert!(story.sprint_name.is_none());
        let issues = validate_story(&story, &features);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();
        assert!(
            !rules.contains(&"missing-field:sprint"),
            "sprint must not be required when the sprints feature is off"
        );
    }

    #[test]
    fn validate_repository_skips_sprint_readmes_when_feature_disabled() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "features.sprints", "false").unwrap();
        set_config_value(temp_root.path(), "paths.sprints", "").unwrap();

        // A stale sprint file is left on disk to prove the validator ignores it.
        let stale_sprint = temp_root.path().join("delivery/sprints/S001.foundation.md");
        fs::create_dir_all(stale_sprint.parent().unwrap()).unwrap();
        fs::write(
            &stale_sprint,
            "---\nsprint: S001\nheadline: foundation\nstart_date: 2099-06-01\nend_date: 2099-06-12\nstatus: not-a-real-status\nwip_limit: ~\n---\n# S001\n",
        )
        .unwrap();

        let report = validate_repository(temp_root.path()).unwrap();
        let rules: Vec<&str> = report.issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules
                .iter()
                .any(|rule| rule.starts_with("missing-sprint-readme-field")
                    || *rule == "invalid-sprint-readme-status"),
            "sprint readme validation must be skipped when sprints are disabled"
        );
    }

    #[test]
    fn validate_story_skips_epic_field_when_epics_feature_disabled() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "features.epics", "false").unwrap();

        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-091-no-epic.md");
        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-091\ntype: user-story\nstatus: todo\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(story_path, temp_root.path()).unwrap();
        let config = load_kanban_config(temp_root.path()).unwrap();
        let features = config.features();
        assert!(!features.epics);
        let issues = validate_story(&story, &features);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();
        assert!(
            !rules.contains(&"missing-field:epic"),
            "epic must not be required when the epics feature is off"
        );
    }

    #[test]
    fn validate_story_rejects_unsafe_task_file_traversal() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-070-traversal.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-070\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 3\ntask_file: ../../../etc/passwd\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(&story_path, temp_root.path()).unwrap();
        let issues = validate_story(&story, &FeaturesConfig::default());
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(
            rules.contains(&"invalid-task-file-path"),
            "expected invalid-task-file-path for traversal task_file"
        );
    }

    #[test]
    fn validate_story_rejects_absolute_and_separator_task_file_values() {
        for value in ["/etc/passwd", "subdir/evil.tasks.md", "C:\\evil.md"] {
            let temp_root = tempdir().unwrap();
            init_temp_repo(temp_root.path());
            let story_path = temp_root
                .path()
                .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-071-task-file.md");

            fs::create_dir_all(story_path.parent().unwrap()).unwrap();
            fs::write(
                &story_path,
                format!(
                    "---\nid: US-F1-071\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 3\ntask_file: {value}\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n"
                ),
            )
            .unwrap();

            let story = read_story_file(&story_path, temp_root.path()).unwrap();
            let issues = validate_story(&story, &FeaturesConfig::default());
            let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();
            assert!(
                rules.contains(&"invalid-task-file-path"),
                "expected invalid-task-file-path for task_file={value:?}"
            );
        }
    }

    #[test]
    fn read_story_file_does_not_read_outside_backlog_for_unsafe_task_file() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let outside = temp_root.path().join("outside-target.txt");
        fs::write(&outside, "SECRET").unwrap();

        let story_path = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-072-escape.md");
        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        // task_file points outside the temp root via `..`. The file exists on
        // disk, but the read path must refuse to canonicalize-and-read it.
        let relative_escape = pathdiff_from(story_path.parent().unwrap(), &outside);
        fs::write(
            &story_path,
            format!(
                "---\nid: US-F1-072\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 3\ntask_file: {relative_escape}\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n"
            ),
        )
        .unwrap();

        let story = read_story_file(&story_path, temp_root.path()).unwrap();
        let task_file = story.task_file.expect("task file metadata is present");
        assert!(
            !task_file.exists,
            "unsafe task_file must not be read outside the backlog root"
        );
        assert!(
            task_file.tasks.is_empty(),
            "no task contents must leak from outside the backlog root"
        );
    }

    #[test]
    fn read_story_file_does_not_follow_symlinked_task_file_escape() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let outside_dir = temp_root.path().join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        let outside_target = outside_dir.join("evil.tasks.md");
        fs::write(&outside_target, "# Tasks for SECRET\n").unwrap();

        let story_dir = temp_root
            .path()
            .join("delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling");
        fs::create_dir_all(&story_dir).unwrap();
        let link_path = story_dir.join("linked.tasks.md");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside_target, &link_path).unwrap();
        }
        #[cfg(not(unix))]
        {
            // Symlink escape is Unix-specific; skip the assertion elsewhere.
            fs::write(&link_path, "placeholder").unwrap();
        }

        let story_path = story_dir.join("US-F1-073-symlink.md");
        fs::write(
            &story_path,
            "---\nid: US-F1-073\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 3\ntask_file: linked.tasks.md\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story\n",
        )
        .unwrap();

        let story = read_story_file(&story_path, temp_root.path()).unwrap();
        let task_file = story.task_file.expect("task file metadata is present");
        #[cfg(unix)]
        {
            assert!(
                !task_file.exists,
                "symlinked task_file escaping the backlog root must not be read"
            );
        }
        let _ = task_file;
    }

    /// Compute a relative `../...` style path from `from` to `to`, for fixtures.
    #[test]
    fn validate_repository_reports_duplicate_story_ids() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let dir = temp_root.path().join("delivery/backlog/phase-1");
        fs::create_dir_all(&dir).unwrap();

        let story_a = dir.join("US-001-alpha.md");
        let story_b = dir.join("US-001-beta.md");
        let frontmatter = "---\nid: US-001\ntype: user-story\nstatus: todo\nepic: EP-001\nsprint: ~\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-06-24T08:00:00+0200\nupdated: 2026-06-24T08:00:00+0200\n---\n# Story\n";
        fs::write(&story_a, frontmatter).unwrap();
        fs::write(&story_b, frontmatter).unwrap();

        let report = validate_repository(temp_root.path()).unwrap();
        let dup_issues: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.rule == "duplicate-story-id")
            .collect();
        assert_eq!(
            dup_issues.len(),
            2,
            "expected 2 duplicate-story-id issues (one per file), got {}",
            dup_issues.len()
        );
    }

    #[test]
    fn validate_repository_reports_out_of_tree_story_path() {
        // US-026: verify the out-of-tree-path check fires when a story's
        // canonicalized file path is outside the backlog root. We test this
        // at the validate_story_with_config level by constructing a story
        // whose file_path resolves outside the config's backlog_path.
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let config = load_kanban_config(temp_root.path()).unwrap();

        // Create a story file outside the backlog root.
        let outside_dir = temp_root.path().join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        let outside_file = outside_dir.join("US-002-escaped.md");
        fs::write(
            &outside_file,
            "---\nid: US-002\ntype: user-story\nstatus: todo\nepic: EP-001\nsprint: ~\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-06-24T08:00:00+0200\nupdated: 2026-06-24T08:00:00+0200\n---\n# Escaped\n",
        )
        .unwrap();

        let story = read_story_file(&outside_file, temp_root.path()).unwrap();
        let issues = validate_story_with_config(&story, &config.features(), Some(&config));
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            rules.contains(&"out-of-tree-story-path"),
            "expected out-of-tree-story-path issue for a story outside the backlog root, got rules: {rules:?}"
        );
    }

    fn pathdiff_from(from: &Path, to: &Path) -> String {
        let mut components = Vec::new();
        let from_parts = from.components().collect::<Vec<_>>();
        let to_parts = to.components().collect::<Vec<_>>();
        // Strip common prefix.
        let mut idx = 0;
        while idx < from_parts.len() && idx < to_parts.len() && from_parts[idx] == to_parts[idx] {
            idx += 1;
        }
        for _ in idx..from_parts.len() {
            components.push("..".to_string());
        }
        for part in &to_parts[idx..] {
            components.push(part.as_os_str().to_string_lossy().into_owned());
        }
        components.join("/")
    }
}
