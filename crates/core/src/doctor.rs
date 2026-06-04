use crate::config::*;
use crate::constants::*;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::sprint::*;
use crate::util::*;
use crate::validate::*;

pub fn collect_doctor_issues(repo_root: impl AsRef<Path>) -> Result<Vec<DoctorIssue>> {
    collect_doctor_issues_at_date(repo_root, Local::now().date_naive())
}

pub fn collect_doctor_issues_for_story(
    repo_root: impl AsRef<Path>,
    story_id: &str,
) -> Result<Vec<DoctorIssue>> {
    let normalized_story_id = story_id.trim().to_ascii_uppercase();
    let issues = collect_doctor_issues(repo_root)?;
    Ok(issues
        .into_iter()
        .filter(|issue| issue.story_id.as_deref() == Some(normalized_story_id.as_str()))
        .collect())
}

pub fn collect_doctor_issues_for_current_sprint(
    repo_root: impl AsRef<Path>,
) -> Result<Vec<DoctorIssue>> {
    let repo_root = repo_root.as_ref();
    let current_sprint = summarize_current_sprint(repo_root)?;
    let sprint_name = current_sprint.sprint_name.clone();
    let current_story_ids = flatten_sprint_stories(&current_sprint)
        .into_iter()
        .map(|story| story.id)
        .collect::<BTreeSet<_>>();
    let issues = collect_doctor_issues(repo_root)?;
    Ok(issues
        .into_iter()
        .filter(|issue| {
            issue.sprint_name.as_deref() == Some(sprint_name.as_str())
                || issue
                    .story_id
                    .as_ref()
                    .is_some_and(|story_id| current_story_ids.contains(story_id))
        })
        .collect())
}

pub fn doctor_repository(repo_root: impl AsRef<Path>) -> Result<Vec<DoctorFinding>> {
    doctor_repository_at_date(repo_root, Local::now().date_naive())
}

pub fn doctor_repository_at_date(
    repo_root: impl AsRef<Path>,
    today: NaiveDate,
) -> Result<Vec<DoctorFinding>> {
    Ok(collect_doctor_issues_at_date(repo_root, today)?
        .into_iter()
        .map(|issue| DoctorFinding {
            severity: issue.severity,
            scope: issue.scope,
            message: issue.message,
        })
        .collect())
}

pub fn apply_doctor_fix(
    repo_root: impl AsRef<Path>,
    issue: &DoctorIssue,
    input: &DoctorFixInput,
) -> Result<DoctorFixResult> {
    let repo_root = resolve_repo_root(repo_root)?;
    let Some(file_path) = &issue.file_path else {
        bail!("Doctor issue cannot be fixed automatically: {}", issue.rule);
    };
    let absolute_path = repo_root.join(file_path);

    match issue.rule.as_str() {
        "missing-field:assignee" => {
            let assignee = input
                .value
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(current_git_assignee(&repo_root)?);
            let validated = validate_assignee_override(&assignee)?;
            upsert_story_frontmatter_file(
                &absolute_path,
                &[("assignee", Some(validated.clone()))],
            )?;
            Ok(DoctorFixResult {
                message: format!("Set assignee to {validated}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        "missing-work-started" => {
            let timestamp = doctor_timestamp_input_with_preview(issue, input)?;
            upsert_story_frontmatter_file(
                &absolute_path,
                &[("work_started", Some(timestamp.clone()))],
            )?;
            Ok(DoctorFixResult {
                message: format!("Set work_started to {timestamp}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        "missing-work-done" => {
            let timestamp = doctor_timestamp_input_with_preview(issue, input)?;
            upsert_story_frontmatter_file(
                &absolute_path,
                &[("work_done", Some(timestamp.clone()))],
            )?;
            Ok(DoctorFixResult {
                message: format!("Set work_done to {timestamp}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        rule if rule.starts_with("invalid-timestamp:") => {
            let field_name = rule.trim_start_matches("invalid-timestamp:");
            let date_only_timestamp =
                date_only_timestamp_from_issue_or_file(issue, input, &absolute_path, field_name)?;
            let corrected_date_only = date_only_timestamp.is_some();
            let timestamp = date_only_timestamp.unwrap_or(doctor_timestamp_input(input)?);
            upsert_story_frontmatter_file(
                &absolute_path,
                &[(field_name, Some(timestamp.clone()))],
            )?;
            Ok(DoctorFixResult {
                message: if corrected_date_only {
                    format!(
                        "INFO: Corrected {field_name} to date-only midnight timestamp {timestamp}."
                    )
                } else {
                    format!("Set {field_name} to {timestamp}.")
                },
                touched_paths: vec![file_path.clone()],
            })
        }
        "missing-task-file" => {
            let story = read_story_file(&absolute_path, &repo_root)?;
            let task_file_name = story
                .frontmatter
                .get("task_file")
                .cloned()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("Sprint story is missing task_file frontmatter."))?;
            let task_file_path = absolute_path.parent().unwrap().join(&task_file_name);
            let story_id = story.frontmatter.get("id").cloned().unwrap_or_default();
            let sprint_name = story.frontmatter.get("sprint").cloned().unwrap_or_default();
            fs::write(
                &task_file_path,
                render_empty_task_file(&story_id, &sprint_name),
            )
            .with_context(|| format!("write task file {}", task_file_path.display()))?;
            Ok(DoctorFixResult {
                message: format!("Created missing task file {task_file_name}."),
                touched_paths: vec![
                    file_path.clone(),
                    relative_path(&repo_root, &task_file_path),
                ],
            })
        }
        rule if rule.starts_with("missing-sprint-readme-field:") => {
            let field_name = rule.trim_start_matches("missing-sprint-readme-field:");
            let readme_update =
                doctor_readme_field_value(&repo_root, &absolute_path, field_name, input)?;
            upsert_story_frontmatter_file(
                &absolute_path,
                &[(field_name, Some(readme_update.clone()))],
            )?;
            Ok(DoctorFixResult {
                message: format!("Set sprint README field {field_name} to {readme_update}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        "sprint-readme-folder-mismatch:sprint" => {
            let file_name = absolute_path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .ok_or_else(|| {
                    anyhow!("Cannot determine sprint file for {}", file_path.display())
                })?;
            let sprint_id = if let Some(value) = input.value.as_ref() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("Sprint README sprint field cannot be empty.");
                }
                trimmed.to_string()
            } else {
                issue
                    .fix_preview
                    .as_ref()
                    .map(|preview| preview.new_value.clone())
                    .or_else(|| parse_sprint_file_name(&file_name).map(|(sprint_id, _)| sprint_id))
                    .ok_or_else(|| anyhow!("Invalid sprint file name: {file_name}"))?
            };
            upsert_story_frontmatter_file(&absolute_path, &[("sprint", Some(sprint_id.clone()))])?;
            Ok(DoctorFixResult {
                message: format!("Aligned sprint README sprint field to {sprint_id}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        "sprint-readme-folder-mismatch:headline" => {
            let file_name = absolute_path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .ok_or_else(|| {
                    anyhow!("Cannot determine sprint file for {}", file_path.display())
                })?;
            let headline = if let Some(value) = input.value.as_ref() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("Sprint README headline field cannot be empty.");
                }
                trimmed.to_string()
            } else {
                issue
                    .fix_preview
                    .as_ref()
                    .map(|preview| preview.new_value.clone())
                    .or_else(|| parse_sprint_file_name(&file_name).map(|(_, headline)| headline))
                    .ok_or_else(|| anyhow!("Invalid sprint file name: {file_name}"))?
            };
            upsert_story_frontmatter_file(&absolute_path, &[("headline", Some(headline.clone()))])?;
            Ok(DoctorFixResult {
                message: format!("Aligned sprint README headline to {headline}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        rule if rule.starts_with("invalid-sprint-readme-date:") => {
            let field_name = rule.trim_start_matches("invalid-sprint-readme-date:");
            let value = input
                .value
                .clone()
                .filter(|candidate| parse_markdown_date(candidate).is_some())
                .ok_or_else(|| anyhow!("Enter a date as YYYY-MM-DD."))?;
            upsert_story_frontmatter_file(&absolute_path, &[(field_name, Some(value.clone()))])?;
            Ok(DoctorFixResult {
                message: format!("Set sprint README {field_name} to {value}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        "invalid-sprint-readme-status"
        | "sprint-readme-status-not-active"
        | "sprint-readme-dates-outside-active" => {
            let value = input.value.clone().unwrap_or_else(|| "active".to_string());
            if !SPRINT_STATUSES.contains(&value.as_str()) {
                bail!("Sprint README status must be one of planned, active, closed, or cancelled.");
            }
            upsert_story_frontmatter_file(&absolute_path, &[("status", Some(value.clone()))])?;
            Ok(DoctorFixResult {
                message: format!("Set sprint README status to {value}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        "roster-drift" => {
            let config = load_kanban_config(&repo_root)?;
            let sprint_name = issue
                .sprint_name
                .as_deref()
                .ok_or_else(|| anyhow!("Roster drift issue is missing sprint name."))?;
            regenerate_sprint_roster(&config, sprint_name)?;
            Ok(DoctorFixResult {
                message: format!("Regenerated roster for {sprint_name}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        rule if rule.starts_with("missing-field:") => {
            let field_name = rule.trim_start_matches("missing-field:");
            let value = input
                .value
                .clone()
                .ok_or_else(|| anyhow!("A value is required for {field_name}."))?;
            upsert_story_frontmatter_file(&absolute_path, &[(field_name, Some(value.clone()))])?;
            Ok(DoctorFixResult {
                message: format!("Set {field_name} to {value}."),
                touched_paths: vec![file_path.clone()],
            })
        }
        _ => bail!("Doctor issue is not auto-fixable: {}", issue.rule),
    }
}

pub(crate) fn collect_doctor_issues_at_date(
    repo_root: impl AsRef<Path>,
    today: NaiveDate,
) -> Result<Vec<DoctorIssue>> {
    let repository = read_repository(repo_root)?;
    let validation = validate_repository(&repository.repo_root)?;
    let config = load_kanban_config(&repository.repo_root)?;
    let sprint_specs = discover_sprint_folder_specs(&config)?;
    let mut findings = Vec::new();

    let stories_by_path = repository
        .stories
        .iter()
        .map(|story| (story.relative_path.clone(), story))
        .collect::<BTreeMap<_, _>>();

    for issue in validation.issues {
        let story = stories_by_path.get(&issue.file_path).copied();
        findings.push(doctor_issue_from_validation(
            &repository.repo_root,
            story,
            &issue,
        ));
    }

    let sprint_names = sprint_specs
        .iter()
        .map(|spec| spec.sprint_name.clone())
        .collect::<BTreeSet<_>>();

    for story in &repository.stories {
        let story_id = story.frontmatter.get("id").cloned();
        let sprint_name = story
            .frontmatter
            .get("sprint")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            .cloned();
        let status = story
            .frontmatter
            .get("status")
            .map(String::as_str)
            .unwrap_or_default();

        if let Some(sprint_name) = sprint_name.as_ref()
            && !sprint_names.contains(sprint_name)
        {
            findings.push(DoctorIssue {
                severity: "error".to_string(),
                scope: story.relative_path.display().to_string(),
                file_path: Some(story.relative_path.clone()),
                story_id: story_id.clone(),
                sprint_name: Some(sprint_name.clone()),
                rule: "orphan-sprint-ref".to_string(),
                message: format!(
                    "Story references sprint `{sprint_name}`, but no matching sprint file exists."
                ),
                suggestion: "Update the story `sprint` field or create the missing sprint file."
                    .to_string(),
                fix_preview: None,
                fix_kind: DoctorFixKind::ManualOnly,
                prompt: DoctorPrompt::None,
            });
        }

        if matches!(
            status,
            "todo" | "in-progress" | "ready-for-qa" | "done" | "blocked"
        ) && sprint_name.is_none()
        {
            findings.push(DoctorIssue {
                severity: "error".to_string(),
                scope: story.relative_path.display().to_string(),
                file_path: Some(story.relative_path.clone()),
                story_id: story_id.clone(),
                sprint_name: None,
                rule: "status-without-sprint".to_string(),
                message: format!(
                    "Story is in board status `{status}` but has no sprint assignment."
                ),
                suggestion:
                    "Assign the story to a sprint or move it back to a backlog-only status."
                        .to_string(),
                fix_preview: None,
                fix_kind: DoctorFixKind::ManualOnly,
                prompt: DoctorPrompt::None,
            });
        }
    }

    let current_by_date: Vec<_> = sprint_specs
        .iter()
        .filter(|spec| date_in_range(today, spec.start_date, spec.end_date))
        .collect();

    if current_by_date.is_empty() {
        findings.push(DoctorIssue {
            severity: "warning".to_string(),
            scope: "sprints".to_string(),
            file_path: None,
            story_id: None,
            sprint_name: None,
            rule: "missing-current-sprint".to_string(),
            message: format!(
                "No sprint folder date range includes {}. Current sprint detection cannot succeed until sprint dates are corrected.",
                today.format("%Y-%m-%d")
            ),
            suggestion: "Select the sprint that should be current and update its README dates or status.".to_string(),
            fix_preview: None,
            fix_kind: DoctorFixKind::ManualOnly,
            prompt: DoctorPrompt::None,
        });
    }

    if current_by_date.len() > 1 {
        findings.push(DoctorIssue {
            severity: "error".to_string(),
            scope: "sprints".to_string(),
            file_path: None,
            story_id: None,
            sprint_name: None,
            rule: "multiple-current-sprints".to_string(),
            message: format!(
                "Multiple sprint folders include {}: {}.",
                today.format("%Y-%m-%d"),
                current_by_date
                    .iter()
                    .map(|spec| spec.sprint_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            suggestion: "Choose which sprint should stay current, then update the other sprint README dates so only one range includes today.".to_string(),
            fix_preview: None,
            fix_kind: DoctorFixKind::ManualOnly,
            prompt: DoctorPrompt::None,
        });
    }

    for spec in sprint_specs {
        findings.extend(doctor_findings_for_sprint(
            &repository.repo_root,
            &repository,
            &spec,
            today,
        ));
    }

    Ok(findings)
}

pub(crate) fn doctor_issue_from_validation(
    repo_root: &Path,
    story: Option<&Story>,
    issue: &ValidationIssue,
) -> DoctorIssue {
    let (suggestion, fix_kind, prompt) = doctor_suggestion_for_validation(repo_root, story, issue);
    let fix_preview = doctor_fix_preview_for_validation(repo_root, story, issue);
    let severity = if date_only_timestamp_issue(story, issue) {
        "info"
    } else {
        "error"
    };
    DoctorIssue {
        severity: severity.to_string(),
        scope: issue.file_path.display().to_string(),
        file_path: Some(issue.file_path.clone()),
        story_id: story.and_then(|story| story.frontmatter.get("id").cloned()),
        sprint_name: story.and_then(|story| story.sprint_name.clone()),
        rule: issue.rule.clone(),
        message: format!("[{}] {}", issue.rule, issue.message),
        suggestion,
        fix_preview,
        fix_kind,
        prompt,
    }
}

pub(crate) fn doctor_fix_preview_for_validation(
    repo_root: &Path,
    story: Option<&Story>,
    issue: &ValidationIssue,
) -> Option<DoctorFixPreview> {
    let story = story?;
    match issue.rule.as_str() {
        "missing-field:assignee" => {
            frontmatter_fix_preview(story, "assignee", current_git_assignee(repo_root).ok()?)
        }
        "missing-work-started" => {
            frontmatter_fix_preview(story, "work_started", current_timestamp_string())
        }
        "missing-work-done" => {
            frontmatter_fix_preview(story, "work_done", current_timestamp_string())
        }
        rule if rule.starts_with("invalid-timestamp:") => {
            let field_name = rule.trim_start_matches("invalid-timestamp:");
            let old_value = story.frontmatter.get(field_name)?;
            let new_value = date_only_timestamp(old_value).unwrap_or_else(current_timestamp_string);
            frontmatter_fix_preview(story, field_name, new_value)
        }
        "sprint-name-mismatch" => {
            frontmatter_fix_preview(story, "sprint", story.sprint_name.clone()?)
        }
        _ => None,
    }
}

pub(crate) fn frontmatter_fix_preview(
    story: &Story,
    field_name: &str,
    new_value: String,
) -> Option<DoctorFixPreview> {
    Some(DoctorFixPreview {
        field_name: field_name.to_string(),
        old_value: story
            .frontmatter
            .get(field_name)
            .cloned()
            .unwrap_or_default(),
        new_value,
    })
}

pub(crate) fn date_only_timestamp_issue(story: Option<&Story>, issue: &ValidationIssue) -> bool {
    let Some(field_name) = issue.rule.strip_prefix("invalid-timestamp:") else {
        return false;
    };
    story
        .and_then(|story| story.frontmatter.get(field_name))
        .and_then(|value| date_only_timestamp(value))
        .is_some()
}

pub(crate) fn doctor_suggestion_for_validation(
    repo_root: &Path,
    story: Option<&Story>,
    issue: &ValidationIssue,
) -> (String, DoctorFixKind, DoctorPrompt) {
    match issue.rule.as_str() {
        "missing-field:assignee" => (
            "Set assignee from git config or enter the correct `Name <email>` value.".to_string(),
            DoctorFixKind::Guided,
            DoctorPrompt::Text {
                label: "Assignee".to_string(),
                default: current_git_assignee(repo_root).ok(),
            },
        ),
        "missing-work-started" => (
            "Set `work_started` to the current local ISO 8601 timestamp.".to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        "missing-work-done" => (
            "Set `work_done` to the current local ISO 8601 timestamp.".to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        rule if rule.starts_with("invalid-timestamp:") => {
            let field_name = rule.trim_start_matches("invalid-timestamp:");
            if story
                .and_then(|story| story.frontmatter.get(field_name))
                .and_then(|value| date_only_timestamp(value))
                .is_some()
            {
                (
                    format!(
                        "Normalize `{field_name}` from `YYYY-MM-DD` to local midnight timestamp."
                    ),
                    DoctorFixKind::Automatic,
                    DoctorPrompt::None,
                )
            } else {
                (
                    format!("Replace `{field_name}` with a valid local ISO 8601 timestamp."),
                    DoctorFixKind::Guided,
                    DoctorPrompt::Text {
                        label: format!("{field_name} timestamp"),
                        default: Some(current_timestamp_string()),
                    },
                )
            }
        }
        "missing-task-file" => (
            "Create the referenced sibling `.tasks.md` file with the standard task log header."
                .to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        rule if rule.starts_with("missing-sprint-readme-field:") => {
            let field_name = rule.trim_start_matches("missing-sprint-readme-field:");
            (
                format!("Insert the missing sprint README frontmatter field `{field_name}`."),
                DoctorFixKind::Guided,
                doctor_prompt_for_readme_field(story, field_name),
            )
        }
        "sprint-readme-folder-mismatch:sprint" => (
            "Align the sprint README `sprint` field to the folder sprint id.".to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        "sprint-readme-folder-mismatch:headline" => (
            "Align the sprint README `headline` field to the folder headline slug.".to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        rule if rule.starts_with("invalid-sprint-readme-date:") => {
            let field_name = rule.trim_start_matches("invalid-sprint-readme-date:");
            (
                format!("Replace `{field_name}` with a valid `YYYY-MM-DD` date."),
                DoctorFixKind::Guided,
                DoctorPrompt::Text {
                    label: format!("{field_name} date"),
                    default: None,
                },
            )
        }
        "invalid-sprint-readme-status" => (
            "Set the sprint README status to one of `planned`, `active`, `closed`, or `cancelled`."
                .to_string(),
            DoctorFixKind::Guided,
            DoctorPrompt::Choice {
                label: "Sprint README status".to_string(),
                options: SPRINT_STATUSES
                    .iter()
                    .map(|status| (*status).to_string())
                    .collect(),
                default: Some("planned".to_string()),
            },
        ),
        "roster-drift" => (
            "Regenerate the sprint roster from story frontmatter.".to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        "orphan-sprint-ref" | "status-without-sprint" => (
            "Inspect and fix the story's sprint assignment manually.".to_string(),
            DoctorFixKind::ManualOnly,
            DoctorPrompt::None,
        ),
        "missing-source-path" | "invalid-source-path" | "missing-source-story" => (
            "Repair `source_path` manually after confirming which backlog story is authoritative."
                .to_string(),
            DoctorFixKind::ManualOnly,
            DoctorPrompt::None,
        ),
        "missing-sprint-readme" | "invalid-sprint-folder-name" | "duplicated-sprint-metadata" => (
            "Inspect and update the sprint folder or README manually.".to_string(),
            DoctorFixKind::ManualOnly,
            DoctorPrompt::None,
        ),
        rule if rule.starts_with("missing-field:") => {
            let field_name = rule.trim_start_matches("missing-field:");
            (
                format!("Enter a value for the missing `{field_name}` frontmatter field."),
                DoctorFixKind::Guided,
                DoctorPrompt::Text {
                    label: field_name.to_string(),
                    default: None,
                },
            )
        }
        _ => (
            "Review and fix this issue manually in the affected markdown file.".to_string(),
            DoctorFixKind::ManualOnly,
            DoctorPrompt::None,
        ),
    }
}

pub(crate) fn doctor_findings_for_sprint(
    repo_root: &Path,
    repository: &Repository,
    spec: &SprintFolderSpec,
    today: NaiveDate,
) -> Vec<DoctorIssue> {
    let mut findings = Vec::new();
    let in_current_range = date_in_range(today, spec.start_date, spec.end_date);
    let sprint_file_path = relative_path(repo_root, &spec.readme_path);
    let expected_rows = repository
        .stories
        .iter()
        .filter(|story| {
            story.frontmatter.get("sprint").map(String::as_str) == Some(spec.sprint_name.as_str())
        })
        .filter_map(|story| {
            Some((
                story.frontmatter.get("id")?.clone(),
                story.frontmatter.get("status").cloned().unwrap_or_default(),
            ))
        })
        .collect::<Vec<_>>();
    let expected_roster = render_sprint_roster(&expected_rows);

    match (in_current_range, spec.readme_status.as_deref()) {
        (true, Some("active")) => {}
        (true, other) => findings.push(DoctorIssue {
            severity: "warning".to_string(),
            scope: spec.sprint_name.clone(),
            file_path: Some(sprint_file_path.clone()),
            story_id: None,
            sprint_name: Some(spec.sprint_name.clone()),
            rule: "sprint-readme-status-not-active".to_string(),
            message: format!(
                "Sprint README dates include {} but README status is {}. README frontmatter is authoritative. Run `kanban doctor` after updating the sprint README.",
                today.format("%Y-%m-%d"),
                other.unwrap_or("missing")
            ),
            suggestion: "Set the sprint README status to active.".to_string(),
            fix_preview: Some(DoctorFixPreview {
                field_name: "status".to_string(),
                old_value: other.unwrap_or_default().to_string(),
                new_value: "active".to_string(),
            }),
            fix_kind: DoctorFixKind::Automatic,
            prompt: DoctorPrompt::None,
        }),
        (false, Some("active")) => findings.push(DoctorIssue {
            severity: "warning".to_string(),
            scope: spec.sprint_name.clone(),
            file_path: Some(sprint_file_path.clone()),
            story_id: None,
            sprint_name: Some(spec.sprint_name.clone()),
            rule: "sprint-readme-dates-outside-active".to_string(),
            message: format!(
                "README status is active but {} is outside the sprint README date range {}..{}. README frontmatter is authoritative. Run `kanban doctor` after updating the sprint README.",
                today.format("%Y-%m-%d"),
                spec.start_date.format("%Y-%m-%d"),
                spec.end_date.format("%Y-%m-%d")
            ),
            suggestion: "Set the sprint README status to planned or closed, or update the date range.".to_string(),
            fix_preview: Some(DoctorFixPreview {
                field_name: "status".to_string(),
                old_value: "active".to_string(),
                new_value: "planned".to_string(),
            }),
            fix_kind: DoctorFixKind::Guided,
            prompt: DoctorPrompt::Choice {
                label: "Sprint README status".to_string(),
                options: SPRINT_STATUSES.iter().map(|status| (*status).to_string()).collect(),
                default: Some("planned".to_string()),
            },
        }),
        _ => {}
    }

    if let Ok(markdown) = fs::read_to_string(&spec.readme_path) {
        let actual_roster = markdown
            .find(ROSTER_HEADING)
            .map(|index| markdown[index..].trim_end().to_string())
            .unwrap_or_default();
        if actual_roster != expected_roster.trim_end() {
            findings.push(DoctorIssue {
                severity: "warning".to_string(),
                scope: spec.sprint_name.clone(),
                file_path: Some(sprint_file_path),
                story_id: None,
                sprint_name: Some(spec.sprint_name.clone()),
                rule: "roster-drift".to_string(),
                message: format!(
                    "Sprint roster in `{}` does not match current story frontmatter.",
                    spec.sprint_name
                ),
                suggestion: "Run the doctor fix or `kanban sprint sync` to regenerate the roster."
                    .to_string(),
                fix_preview: None,
                fix_kind: DoctorFixKind::Automatic,
                prompt: DoctorPrompt::None,
            });
        }
    }

    findings
}

pub(crate) fn doctor_timestamp_input(input: &DoctorFixInput) -> Result<String> {
    let timestamp = input.value.clone().unwrap_or_else(current_timestamp_string);
    validate_doctor_timestamp(&timestamp)?;
    Ok(timestamp)
}

pub(crate) fn doctor_timestamp_input_with_preview(
    issue: &DoctorIssue,
    input: &DoctorFixInput,
) -> Result<String> {
    let timestamp = input
        .value
        .clone()
        .or_else(|| {
            issue
                .fix_preview
                .as_ref()
                .map(|preview| preview.new_value.clone())
        })
        .unwrap_or_else(current_timestamp_string);
    validate_doctor_timestamp(&timestamp)?;
    Ok(timestamp)
}

pub(crate) fn validate_doctor_timestamp(timestamp: &str) -> Result<()> {
    let timestamp_pattern = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{4}$")
        .expect("valid timestamp regex");
    if !timestamp_pattern.is_match(timestamp) {
        bail!("Enter a timestamp as local ISO 8601 with numeric timezone offset.");
    }
    Ok(())
}

pub(crate) fn date_only_timestamp_from_issue_or_file(
    issue: &DoctorIssue,
    input: &DoctorFixInput,
    file_path: &Path,
    field_name: &str,
) -> Result<Option<String>> {
    if input.value.is_some() {
        return Ok(None);
    }

    if let Some(preview) = &issue.fix_preview
        && preview.field_name == field_name
        && date_only_timestamp(&preview.old_value).is_some()
    {
        return Ok(Some(preview.new_value.clone()));
    }

    date_only_timestamp_from_file(file_path, field_name)
}

pub(crate) fn doctor_prompt_for_readme_field(
    story: Option<&Story>,
    field_name: &str,
) -> DoctorPrompt {
    match field_name {
        "status" => DoctorPrompt::Choice {
            label: "Sprint README status".to_string(),
            options: SPRINT_STATUSES
                .iter()
                .map(|status| (*status).to_string())
                .collect(),
            default: Some("planned".to_string()),
        },
        "start_date" | "end_date" => DoctorPrompt::Text {
            label: format!("{field_name} date"),
            default: None,
        },
        "sprint" => DoctorPrompt::Text {
            label: "Sprint id".to_string(),
            default: story.and_then(|story| {
                story
                    .file_path
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .and_then(|file_name| {
                        parse_sprint_file_name(&file_name).map(|(sprint, _)| sprint)
                    })
            }),
        },
        "headline" => DoctorPrompt::Text {
            label: "Sprint headline".to_string(),
            default: story.and_then(|story| {
                story
                    .file_path
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .and_then(|file_name| {
                        parse_sprint_file_name(&file_name).map(|(_, headline)| headline)
                    })
            }),
        },
        _ => DoctorPrompt::Text {
            label: field_name.to_string(),
            default: None,
        },
    }
}

pub(crate) fn doctor_readme_field_value(
    repo_root: &Path,
    readme_path: &Path,
    field_name: &str,
    input: &DoctorFixInput,
) -> Result<String> {
    if let Some(value) = input.value.clone().filter(|value| !value.trim().is_empty()) {
        return Ok(value);
    }

    let file_name = readme_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned());
    let parsed_file = file_name.as_deref().and_then(parse_sprint_file_name);

    let value = match field_name {
        "sprint" => parsed_file
            .map(|(sprint, _)| sprint)
            .ok_or_else(|| anyhow!("Enter the sprint id for this README."))?,
        "headline" => parsed_file
            .map(|(_, headline)| headline)
            .ok_or_else(|| anyhow!("Enter the sprint headline for this README."))?,
        "status" => "planned".to_string(),
        "start_date" | "end_date" => {
            bail!("Enter a date as YYYY-MM-DD.");
        }
        "wip_limit" => "null".to_string(),
        other => bail!("Cannot derive sprint README field {other} automatically."),
    };

    let _ = repo_root;
    Ok(value)
}
