use crate::config::*;
use crate::constants::*;
use crate::epic::*;
use crate::lock::RepoLock;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::sprint::*;
use crate::story::*;
use crate::util::*;
use crate::validate::*;

fn upsert_epic_frontmatter_file(
    file_path: &Path,
    updates: &[(&str, Option<String>)],
) -> Result<()> {
    let markdown = fs::read_to_string(file_path)
        .with_context(|| format!("read epic file {}", file_path.display()))?;
    let updated = upsert_frontmatter_markdown(&markdown, updates)?;
    atomic_write(file_path, &updated)
        .with_context(|| format!("write epic file {}", file_path.display()))?;
    Ok(())
}

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
    let _lock = RepoLock::acquire(&repo_root)?;
    let Some(file_path) = &issue.file_path else {
        bail!("Doctor issue cannot be fixed automatically: {}", issue.rule);
    };
    let absolute_path = repo_root.join(file_path);
    // Containment check: refuse to write to any path that resolves outside the
    // canonicalized backlog root (US-008 scenario 2). Bails before any write so
    // no out-of-tree file is created or modified.
    let absolute_path = ensure_path_inside(&repo_root, &absolute_path)?;

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
        "planned-status-in-sprint" => {
            let story = read_story_file(&absolute_path, &repo_root)?;
            upsert_story_frontmatter_file(&absolute_path, &[("status", Some("todo".to_string()))])?;
            if let Some(sprint_name) = story
                .frontmatter
                .get("sprint")
                .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            {
                let config = load_kanban_config(&repo_root)?;
                regenerate_sprint_roster(&config, sprint_name)?;
            }
            Ok(DoctorFixResult {
                message: "Changed story status from planned to todo.".to_string(),
                touched_paths: vec![file_path.clone()],
            })
        }
        rule if rule.starts_with("invalid-timestamp:") => {
            let field_name = rule.trim_start_matches("invalid-timestamp:");
            let automatic_fix = automatic_timestamp_fix_from_issue_or_file(
                issue,
                input,
                &absolute_path,
                field_name,
            )?;
            let corrected_kind = automatic_fix.as_ref().map(|fix| fix.kind());
            let corrected_zulu_input = input
                .value
                .as_deref()
                .is_some_and(|value| zulu_timestamp(value).is_some());
            let timestamp = automatic_fix
                .map(|fix| fix.timestamp().to_string())
                .unwrap_or(doctor_timestamp_input(input)?);
            upsert_story_frontmatter_file(
                &absolute_path,
                &[(field_name, Some(timestamp.clone()))],
            )?;
            Ok(DoctorFixResult {
                message: if corrected_kind == Some("date-only") {
                    format!(
                        "INFO: Corrected {field_name} to date-only midnight timestamp {timestamp}."
                    )
                } else if corrected_kind == Some("Zulu") || corrected_zulu_input {
                    format!(
                        "INFO: Corrected {field_name} from Zulu timestamp to local timestamp {timestamp}."
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
            validate_task_file_frontmatter_value(&task_file_name)?;
            let parent = absolute_path.parent().with_context(|| {
                format!(
                    "story file {} has no parent directory",
                    absolute_path.display()
                )
            })?;
            let task_file_path = parent.join(&task_file_name);
            let backlog_root = load_kanban_config(&repo_root)?.backlog_path();
            let task_file_path = ensure_path_inside(&backlog_root, &task_file_path)?;
            let story_id = story.frontmatter.get("id").cloned().unwrap_or_default();
            let sprint_name = story.frontmatter.get("sprint").cloned().unwrap_or_default();
            atomic_write(
                &task_file_path,
                &render_empty_task_file(&story_id, &sprint_name),
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
        "legacy-task-file-format" => {
            let story_file_name = absolute_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| format!("{}.md", value.trim_end_matches(TASK_FILE_SUFFIX)))
                .ok_or_else(|| {
                    anyhow!("Cannot determine story file for {}", file_path.display())
                })?;
            let story_path = absolute_path.with_file_name(story_file_name);
            let story = read_story_file(&story_path, &repo_root)?;
            let task_file = story
                .task_file
                .as_ref()
                .ok_or_else(|| anyhow!("Story is missing task file metadata."))?;
            let story_id = story.frontmatter.get("id").cloned().unwrap_or_default();
            let sprint_name = story
                .frontmatter
                .get("sprint")
                .cloned()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "~".to_string());
            atomic_write(
                &task_file.file_path,
                &render_task_file(&story_id, &sprint_name, &task_file.tasks),
            )
            .with_context(|| format!("write task file {}", task_file.file_path.display()))?;
            Ok(DoctorFixResult {
                message: "Rewrote task file to canonical heading-delimited format.".to_string(),
                touched_paths: vec![file_path.clone()],
            })
        }
        "epic-status-lags-active-children" => {
            let value = input
                .value
                .clone()
                .or_else(|| {
                    issue
                        .fix_preview
                        .as_ref()
                        .map(|preview| preview.new_value.clone())
                })
                .unwrap_or_else(|| "in-progress".to_string());
            let normalized = normalize_story_status_input(&value)?;
            upsert_epic_frontmatter_file(&absolute_path, &[("status", Some(normalized.clone()))])?;
            Ok(DoctorFixResult {
                message: format!("Set epic status to {normalized}."),
                touched_paths: vec![file_path.clone()],
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
                message: format!("Regenerated selected-user-story section for {sprint_name}."),
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
    let sprints_enabled = config.features().sprints;
    let sprint_specs = if sprints_enabled {
        discover_sprint_folder_specs(&config)?
    } else {
        Vec::new()
    };
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

    for story in &repository.stories {
        let Some(task_file) = &story.task_file else {
            continue;
        };
        let Some(markdown) = task_file.markdown.as_deref() else {
            continue;
        };
        if !task_file_uses_legacy_separators(markdown) {
            continue;
        }

        findings.push(DoctorIssue {
            severity: "info".to_string(),
            scope: task_file.relative_path.display().to_string(),
            file_path: Some(task_file.relative_path.clone()),
            story_id: story.frontmatter.get("id").cloned(),
            sprint_name: story.sprint_name.clone(),
            rule: "legacy-task-file-format".to_string(),
            message: "Task file uses legacy `---` separators that can be mistaken for Markdown/YAML frontmatter fences by generic tooling.".to_string(),
            suggestion: "Run doctor fix to rewrite the task file to the canonical heading-delimited format.".to_string(),
            fix_preview: None,
            fix_kind: DoctorFixKind::Automatic,
            prompt: DoctorPrompt::None,
        });
    }

    if sprints_enabled {
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

            if SPRINT_STATUS_DISPLAY_ORDER.contains(&status) && sprint_name.is_none() {
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
    }

    if sprints_enabled {
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
    }

    if sprints_enabled {
        for spec in sprint_specs {
            findings.extend(doctor_findings_for_sprint(
                &repository.repo_root,
                &repository,
                &spec,
                today,
            ));
        }
    }

    if config.features().epics {
        for epic_file in collect_epic_files(&repository.repo_root)? {
            let epic = read_epic_file(&epic_file, &repository.repo_root)?;
            let details = find_epic(
                &repository.repo_root,
                epic.frontmatter
                    .get("id")
                    .map(String::as_str)
                    .unwrap_or_default(),
            )?;
            let Some(details) = details else {
                continue;
            };
            if let Some(warning) = epic_status_warning(&details) {
                let suggested_status = if details
                    .stories_by_status
                    .get("in-progress")
                    .is_some_and(|stories| !stories.is_empty())
                {
                    "in-progress"
                } else {
                    "ready-for-qa"
                };
                findings.push(DoctorIssue {
                    severity: "warning".to_string(),
                    scope: details.epic.relative_path.display().to_string(),
                    file_path: Some(details.epic.relative_path.clone()),
                    story_id: None,
                    sprint_name: None,
                    rule: "epic-status-lags-active-children".to_string(),
                    message: warning,
                    suggestion: "Update the epic status so it reflects the most advanced active child story state.".to_string(),
                    fix_preview: Some(DoctorFixPreview {
                        field_name: "status".to_string(),
                        old_value: details.epic.status.clone(),
                        new_value: suggested_status.to_string(),
                    }),
                    fix_kind: DoctorFixKind::Guided,
                    prompt: DoctorPrompt::Choice {
                        label: "Epic status".to_string(),
                        options: CANONICAL_STORY_STATUSES.iter().map(|status| (*status).to_string()).collect(),
                        default: Some(suggested_status.to_string()),
                    },
                });
            }
        }
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
    let severity = if automatic_timestamp_issue(story, issue) {
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
        "planned-status-in-sprint" => frontmatter_fix_preview(story, "status", "todo".to_string()),
        rule if rule.starts_with("invalid-timestamp:") => {
            let field_name = rule.trim_start_matches("invalid-timestamp:");
            let old_value = story.frontmatter.get(field_name)?;
            let new_value = automatic_timestamp_fix(old_value)
                .map(|fix| fix.timestamp().to_string())
                .unwrap_or_else(current_timestamp_string);
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

pub(crate) fn automatic_timestamp_issue(story: Option<&Story>, issue: &ValidationIssue) -> bool {
    let Some(field_name) = issue.rule.strip_prefix("invalid-timestamp:") else {
        return false;
    };
    story
        .and_then(|story| story.frontmatter.get(field_name))
        .and_then(|value| automatic_timestamp_fix(value))
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
        "planned-status-in-sprint" => (
            "Change the story status from `planned` to `todo` because sprint-assigned stories must start in `todo`.".to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        rule if rule.starts_with("invalid-timestamp:") => {
            let field_name = rule.trim_start_matches("invalid-timestamp:");
            if let Some(fix) = story
                .and_then(|story| story.frontmatter.get(field_name))
                .and_then(|value| automatic_timestamp_fix(value))
            {
                let source = match fix {
                    DoctorTimestampFix::DateOnly(_) => "`YYYY-MM-DD` to local midnight timestamp",
                    DoctorTimestampFix::Zulu(_) => "Zulu timestamp to local timestamp",
                };
                (
                    format!("Normalize `{field_name}` from {source}."),
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
        "legacy-task-file-format" => (
            "Rewrite the task file to the canonical heading-delimited format with no `---` separators."
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
            "Regenerate the selected-user-story section from story frontmatter.".to_string(),
            DoctorFixKind::Automatic,
            DoctorPrompt::None,
        ),
        "epic-status-lags-active-children" => (
            "Set the epic status to reflect that one or more child stories are already active.".to_string(),
            DoctorFixKind::Guided,
            DoctorPrompt::Choice {
                label: "Epic status".to_string(),
                options: CANONICAL_STORY_STATUSES
                    .iter()
                    .map(|status| (*status).to_string())
                    .collect(),
                default: Some("in-progress".to_string()),
            },
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
        .map(|story| {
            let overview = story_overview(repo_root, story);
            let link_path =
                sprint_story_link_path(repo_root, &spec.readme_path, &overview.relative_path);
            SprintRosterEntry {
                story: overview,
                link_path,
            }
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
    if let Some(normalized) = zulu_timestamp(&timestamp) {
        return Ok(normalized);
    }
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
    if let Some(normalized) = zulu_timestamp(&timestamp) {
        return Ok(normalized);
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DoctorTimestampFix {
    DateOnly(String),
    Zulu(String),
}

impl DoctorTimestampFix {
    pub(crate) fn timestamp(&self) -> &str {
        match self {
            DoctorTimestampFix::DateOnly(timestamp) | DoctorTimestampFix::Zulu(timestamp) => {
                timestamp
            }
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            DoctorTimestampFix::DateOnly(_) => "date-only",
            DoctorTimestampFix::Zulu(_) => "Zulu",
        }
    }
}

pub(crate) fn automatic_timestamp_fix(value: &str) -> Option<DoctorTimestampFix> {
    date_only_timestamp(value)
        .map(DoctorTimestampFix::DateOnly)
        .or_else(|| zulu_timestamp(value).map(DoctorTimestampFix::Zulu))
}

pub(crate) fn automatic_timestamp_fix_from_issue_or_file(
    issue: &DoctorIssue,
    input: &DoctorFixInput,
    file_path: &Path,
    field_name: &str,
) -> Result<Option<DoctorTimestampFix>> {
    if input.value.is_some() {
        return Ok(None);
    }

    if let Some(preview) = &issue.fix_preview
        && preview.field_name == field_name
        && let Some(fix) = automatic_timestamp_fix(&preview.old_value)
    {
        return Ok(Some(match fix {
            DoctorTimestampFix::DateOnly(_) => {
                DoctorTimestampFix::DateOnly(preview.new_value.clone())
            }
            DoctorTimestampFix::Zulu(_) => DoctorTimestampFix::Zulu(preview.new_value.clone()),
        }));
    }

    let markdown = fs::read_to_string(file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    Ok(parsed
        .frontmatter
        .get(field_name)
        .and_then(|value| automatic_timestamp_fix(value)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use tempfile::tempdir;

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
    fn doctor_fix_normalizes_zulu_timestamp_to_local_timestamp() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let zulu = "2026-05-28T12:05:54.123Z";
        let expected = zulu_timestamp(zulu).unwrap();
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-054-zulu-work-started.md",
            &format!(
                "id: US-F1-054\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started: {zulu}\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n"
            ),
        );

        let issue = collect_doctor_issues_for_story(temp_root.path(), "US-F1-054")
            .unwrap()
            .into_iter()
            .find(|issue| issue.rule == "invalid-timestamp:work_started")
            .unwrap();
        let result =
            apply_doctor_fix(temp_root.path(), &issue, &DoctorFixInput::default()).unwrap();
        let updated = fs::read_to_string(&story_path).unwrap();

        assert_eq!(issue.severity, "info");
        assert!(matches!(issue.fix_kind, DoctorFixKind::Automatic));
        assert!(
            result
                .message
                .contains("INFO: Corrected work_started from Zulu timestamp")
        );
        assert!(updated.contains(&format!("work_started: {expected}")));
    }

    #[test]
    fn doctor_does_not_report_null_work_done_for_unfinished_story() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-055-null-work-done.md",
            "id: US-F1-055\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: ~\nstory_points: 5\nwork_started:\nwork_done: null\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let issues = collect_doctor_issues_for_story(temp_root.path(), "US-F1-055").unwrap();

        assert!(
            issues
                .iter()
                .all(|issue| issue.rule != "invalid-timestamp:work_done")
        );
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
    fn doctor_reports_epic_status_lagging_in_progress_children() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        fs::create_dir_all(
            temp_root
                .path()
                .join("delivery/backlog/phase-1-scaffolding/01.platform"),
        )
        .unwrap();
        fs::write(
            temp_root
                .path()
                .join("delivery/backlog/phase-1-scaffolding/01.platform/EP-F1-01-platform.md"),
            "---\nid: EP-F1-01\ntype: epic\nstatus: draft\nphase: 1\nowner: Owner\nmilestone: MP1\ncreated: 2026-01-01T00:00:00+0200\nupdated: 2026-01-01T00:00:00+0200\n---\n\n# Epic: Platform\n",
        )
        .unwrap();
        write_story(
            temp_root.path(),
            "delivery/backlog/phase-1-scaffolding/01.platform/US-F1-005-secrets.md",
            "id: US-F1-005\ntype: user-story\nstatus: in-progress\nepic: EP-F1-01\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started: 2026-01-01T00:00:00+0200\nwork_done:\ncreated: 2026-01-01T00:00:00+0200\nupdated: 2026-01-01T00:00:00+0200\n",
        );

        let issues = collect_doctor_issues(temp_root.path()).unwrap();
        let issue = issues
            .iter()
            .find(|issue| issue.rule == "epic-status-lags-active-children")
            .expect("expected epic status lag issue");
        assert_eq!(issue.severity, "warning");
        assert!(issue.message.contains("child stories are `in-progress`"));
        assert_eq!(
            issue.fix_preview.as_ref().map(|p| p.new_value.as_str()),
            Some("in-progress")
        );
    }

    #[test]
    fn doctor_reports_and_fixes_planned_status_for_story_in_sprint() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "delivery/backlog/phase-1-scaffolding/01.platform/US-F1-006-planned-in-sprint.md",
            "id: US-F1-006\ntype: user-story\nstatus: planned\nepic: EP-F1-01\nsprint: S001.foundation\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-01-01T00:00:00+0200\nupdated: 2026-01-01T00:00:00+0200\n",
        );

        let issue = collect_doctor_issues_for_story(temp_root.path(), "US-F1-006")
            .unwrap()
            .into_iter()
            .find(|issue| issue.rule == "planned-status-in-sprint")
            .expect("expected planned status in sprint issue");

        assert_eq!(issue.severity, "error");
        assert!(matches!(issue.fix_kind, DoctorFixKind::Automatic));
        assert_eq!(
            issue
                .fix_preview
                .as_ref()
                .map(|preview| preview.new_value.as_str()),
            Some("todo")
        );

        let result =
            apply_doctor_fix(temp_root.path(), &issue, &DoctorFixInput::default()).unwrap();
        let updated = fs::read_to_string(&story_path).unwrap();

        assert_eq!(result.message, "Changed story status from planned to todo.");
        assert!(updated.contains("status: todo"));
    }

    #[test]
    fn doctor_skips_sprint_rules_when_sprints_feature_disabled() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "features.sprints", "false").unwrap();
        set_config_value(temp_root.path(), "paths.sprints", "").unwrap();

        // Story in board status with no sprint — would normally trigger
        // status-without-sprint, but the feature is off.
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-099-orphan.md",
            "id: US-F1-099\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let issues = collect_doctor_issues(temp_root.path()).unwrap();
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"status-without-sprint"),
            "status-without-sprint must not fire when sprints are disabled"
        );
        assert!(
            !rules.contains(&"missing-current-sprint"),
            "missing-current-sprint must not fire when sprints are disabled"
        );
        assert!(
            !rules.contains(&"multiple-current-sprints"),
            "multiple-current-sprints must not fire when sprints are disabled"
        );
    }

    #[test]
    fn doctor_skips_epic_status_rule_when_epics_feature_disabled() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "features.epics", "false").unwrap();

        // Stale epic + in-progress child would normally trigger
        // epic-status-lags-active-children, but the feature is off.
        fs::create_dir_all(
            temp_root
                .path()
                .join("delivery/backlog/phase-1-scaffolding/01.platform"),
        )
        .unwrap();
        fs::write(
            temp_root
                .path()
                .join("delivery/backlog/phase-1-scaffolding/01.platform/EP-F1-01-platform.md"),
            "---\nid: EP-F1-01\ntype: epic\nstatus: draft\nphase: 1\ncreated: 2026-01-01T00:00:00+0200\nupdated: 2026-01-01T00:00:00+0200\n---\n\n# Epic: Platform\n",
        )
        .unwrap();
        write_story(
            temp_root.path(),
            "delivery/backlog/phase-1-scaffolding/01.platform/US-F1-005-secrets.md",
            "id: US-F1-005\ntype: user-story\nstatus: in-progress\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started: 2026-01-01T00:00:00+0200\nwork_done:\ncreated: 2026-01-01T00:00:00+0200\nupdated: 2026-01-01T00:00:00+0200\n",
        );

        let issues = collect_doctor_issues(temp_root.path()).unwrap();
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"epic-status-lags-active-children"),
            "epic-status-lags-active-children must not fire when epics are disabled"
        );
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
    fn doctor_reports_and_fixes_legacy_task_file_format() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-057-task-file-format.md",
            "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n",
        );
        let task_path = story_path.with_extension("tasks.md");
        fs::write(
            &task_path,
            "# Tasks for US-F1-057\n\nParent User Story: US-F1-057\nSprint: ~\n\n---\n\n## TASK-US-F1-057-001 - First task\n\nStatus: To Do\nTags: cli\n\nDescription:\nFirst.\n\n---\n",
        )
        .unwrap();

        let story = read_story_file(&story_path, temp_root.path()).unwrap();
        let task_file = story.task_file.as_ref().expect("task file should be read");
        assert!(
            task_file_uses_legacy_separators(task_file.markdown.as_deref().unwrap()),
            "task file should be detected as legacy format"
        );

        let issues = collect_doctor_issues(temp_root.path()).unwrap();
        assert!(
            issues
                .iter()
                .any(|issue| issue.rule == "legacy-task-file-format"),
            "expected legacy task file issue, got: {:?}",
            issues
                .iter()
                .map(|issue| issue.rule.as_str())
                .collect::<Vec<_>>()
        );
        let issue = issues
            .into_iter()
            .find(|issue| issue.rule == "legacy-task-file-format")
            .unwrap();

        let result =
            apply_doctor_fix(temp_root.path(), &issue, &DoctorFixInput::default()).unwrap();
        let updated = fs::read_to_string(&task_path).unwrap();

        assert!(
            result
                .message
                .contains("canonical heading-delimited format")
        );
        assert!(updated.starts_with("# Tasks for US-F1-057\n\nParent User Story: US-F1-057\nSprint: ~\n\n## TASK-US-F1-057-001 - First task"));
        assert!(!updated.contains("\n---\n"));
        assert!(updated.contains("Status: todo"));
    }

    #[test]
    fn apply_doctor_fix_refuses_to_write_outside_backlog_root() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_git_config(temp_root.path(), "Test User", "test@example.com");

        // Fabricate a doctor issue whose file_path resolves outside the repo
        // root via `..`. The fix must bail before any write occurs.
        let outside = temp_root.path().join("outside-target.md");
        let issue = DoctorIssue {
            severity: "error".to_string(),
            scope: "outside".to_string(),
            file_path: Some(PathBuf::from("../../outside-target.md")),
            story_id: None,
            sprint_name: None,
            rule: "missing-field:assignee".to_string(),
            message: "Missing assignee.".to_string(),
            suggestion: String::new(),
            fix_preview: None,
            fix_kind: DoctorFixKind::Guided,
            prompt: DoctorPrompt::None,
        };

        let err =
            apply_doctor_fix(temp_root.path(), &issue, &DoctorFixInput::default()).unwrap_err();
        assert!(err.to_string().contains("outside the backlog root"));
        assert!(
            !outside.exists(),
            "no file outside the backlog root must be created"
        );
    }
}
