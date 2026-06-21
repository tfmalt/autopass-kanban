use crate::config::*;
use crate::constants::*;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::sprint::*;
use crate::util::*;

pub fn list_all_stories(repo_root: impl AsRef<Path>) -> Result<Vec<StoryOverview>> {
    let repository = read_repository(repo_root)?;
    Ok(unique_story_overviews(&repository))
}

pub fn move_story_to_status(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    target_status: &str,
) -> Result<MoveStoryResult> {
    move_story_to_status_with_assignee(repo_root, story_id, target_status, None)
}

pub fn move_story_to_status_with_assignee(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    target_status: &str,
    assignee_override: Option<&str>,
) -> Result<MoveStoryResult> {
    let config = load_kanban_config(repo_root)?;
    let sprints_enabled = config.features().sprints;
    let repository = read_repository(&config.repo_root)?;
    let normalized_story_id = story_id.trim().to_ascii_uppercase();
    let normalized_status = normalize_story_status_input(target_status)?;
    let assignee_override = match assignee_override {
        Some(_) if normalized_status != "in-progress" => {
            bail!("Assignee override can only be used when moving a story to in-progress.");
        }
        Some(assignee) => Some(validate_assignee_override(assignee)?),
        None => None,
    };
    let story = repository
        .stories
        .iter()
        .find(|story| {
            story
                .frontmatter
                .get("id")
                .map(|id| id.eq_ignore_ascii_case(&normalized_story_id))
                .unwrap_or(false)
        })
        .cloned()
        .ok_or_else(|| anyhow!("Story not found: {normalized_story_id}"))?;

    let sprint_name = if sprints_enabled {
        story
            .frontmatter
            .get("sprint")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            .cloned()
            .ok_or_else(|| anyhow!("Story {normalized_story_id} is not assigned to a sprint."))?
    } else {
        String::new()
    };
    let current_status = story.frontmatter.get("status").cloned().unwrap_or_default();

    let assignee_update = if normalized_status == "in-progress" {
        Some(match assignee_override {
            Some(assignee) => assignee,
            None => match story.frontmatter.get("assignee") {
                Some(existing) if !parse_assignee_list(existing).is_empty() => existing.clone(),
                _ => current_git_assignee(&repository.repo_root)?,
            },
        })
    } else {
        story.frontmatter.get("assignee").cloned()
    };
    let now = current_timestamp_string();
    let work_started_update = if normalized_status == "in-progress" {
        story
            .frontmatter
            .get("work_started")
            .filter(|value| !value.is_empty())
            .cloned()
            .or_else(|| Some(now.clone()))
    } else {
        story.frontmatter.get("work_started").cloned()
    };
    let work_done_update = if normalized_status == "done" {
        Some(now.clone())
    } else {
        story.frontmatter.get("work_done").cloned()
    };

    let story_markdown = update_story_frontmatter_markdown(
        &story.markdown,
        &[
            ("status", Some(normalized_status.clone())),
            ("updated", Some(now.clone())),
            ("assignee", assignee_update.clone()),
            ("work_started", work_started_update),
            ("work_done", work_done_update),
        ],
    )?;
    fs::write(&story.file_path, story_markdown)
        .with_context(|| format!("rewrite story {}", story.file_path.display()))?;
    if sprints_enabled {
        regenerate_sprint_roster(&load_kanban_config(&repository.repo_root)?, &sprint_name)?;
    }

    Ok(MoveStoryResult {
        story_id: normalized_story_id,
        sprint_name,
        from_status: current_status,
        to_status: normalized_status,
        story_path: story.relative_path,
        task_path: story
            .task_file
            .as_ref()
            .map(|task_file| task_file.relative_path.clone()),
    })
}

pub fn plan_story_into_sprint(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    sprint_name: &str,
) -> Result<PlanStoryResult> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().sprints {
        bail!(
            "Sprints are disabled in .kanban/settings.json. Run `kanban features enable sprints` to re-enable them."
        );
    }
    let repo_root = config.repo_root.clone();
    let normalized_story_id = story_id.trim().to_ascii_uppercase();

    let sprint_query = sprint_name.trim();
    if !config.sprints_path().is_dir() {
        bail!("Sprint not found: {sprint_query}");
    }
    let sprint_names = list_sprint_names(&repo_root)?;
    let sprint_name = sprint_names
        .iter()
        .find(|name| name.as_str() == sprint_query)
        .or_else(|| {
            sprint_names
                .iter()
                .find(|name| name.starts_with(&format!("{sprint_query}.")))
        })
        .cloned()
        .ok_or_else(|| anyhow!("Sprint not found: {sprint_query}"))?;

    let repository = read_repository(&repo_root)?;
    let story = repository
        .stories
        .iter()
        .find(|story| {
            story
                .frontmatter
                .get("id")
                .map(|id| id.eq_ignore_ascii_case(&normalized_story_id))
                .unwrap_or(false)
        })
        .cloned()
        .ok_or_else(|| anyhow!("Story not found: {normalized_story_id}"))?;

    let now = current_timestamp_string();
    let activated_now = current_timestamp_string();
    let activated = story
        .frontmatter
        .get("activated")
        .filter(|value| !value.is_empty())
        .cloned()
        .or(Some(activated_now));
    let current_status = story
        .frontmatter
        .get("status")
        .map(String::as_str)
        .unwrap_or_default();
    let current_status_normalized = normalize_status_alias(current_status);
    let new_status = if current_status.is_empty()
        || matches!(current_status_normalized.as_str(), "draft" | "ready")
    {
        "todo"
    } else {
        current_status
    };
    let moved_markdown = upsert_frontmatter_markdown(
        &story.markdown,
        &[
            ("status", Some(new_status.to_string())),
            ("sprint", Some(sprint_name.clone())),
            ("activated", activated),
            ("updated", Some(now)),
        ],
    )?;
    fs::write(&story.file_path, moved_markdown)
        .with_context(|| format!("rewrite planned story {}", story.file_path.display()))?;
    regenerate_sprint_roster(&config, &sprint_name)?;

    Ok(PlanStoryResult {
        story_id: normalized_story_id,
        sprint_name,
        story_path: story.relative_path,
        task_path: story
            .task_file
            .as_ref()
            .map(|task_file| task_file.relative_path.clone()),
    })
}

pub fn delete_story(repo_root: impl AsRef<Path>, story_id: &str) -> Result<DeleteStoryResult> {
    let config = load_kanban_config(repo_root)?;
    let repository = read_repository(&config.repo_root)?;
    let story = find_story_for_write(&repository, story_id)?.clone();
    let story_id_value = story.frontmatter.get("id").cloned().unwrap_or_default();
    let sprint_name = story
        .frontmatter
        .get("sprint")
        .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
        .cloned();
    let story_path = story.relative_path.clone();
    let task_path = story
        .task_file
        .as_ref()
        .map(|task_file| task_file.relative_path.clone());

    if let Some(task_file) = story.task_file.as_ref()
        && task_file.file_path.exists()
    {
        fs::remove_file(&task_file.file_path)
            .with_context(|| format!("delete task file {}", task_file.file_path.display()))?;
    }
    fs::remove_file(&story.file_path)
        .with_context(|| format!("delete story {}", story.file_path.display()))?;

    if let Some(sprint_name) = sprint_name.as_deref() {
        regenerate_sprint_roster(&config, sprint_name)?;
    }

    Ok(DeleteStoryResult {
        story_id: story_id_value,
        sprint_name,
        story_path,
        task_path,
    })
}

pub fn add_task_to_story(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    title: &str,
    status: &str,
    tags: &[String],
    description: &str,
) -> Result<TaskMutationResult> {
    let repository = read_repository(repo_root)?;
    let story = find_story_for_write(&repository, story_id)?;
    let empty_task_file;
    let task_file = if let Some(task_file) = story.task_file.as_ref() {
        task_file
    } else {
        empty_task_file = TaskFile {
            exists: false,
            file_path: story.file_path.with_extension("tasks.md"),
            relative_path: relative_path(
                &repository.repo_root,
                &story.file_path.with_extension("tasks.md"),
            ),
            tasks: Vec::new(),
            summary: TaskSummary::default(),
            markdown: None,
        };
        &empty_task_file
    };
    let task_id = next_task_id(story, task_file);
    let normalized_status = normalize_task_status_for_write(status)?;
    let initial_markdown;
    let markdown = if let Some(markdown) = task_file.markdown.as_deref() {
        markdown
    } else {
        initial_markdown = render_empty_task_file(
            story
                .frontmatter
                .get("id")
                .map(String::as_str)
                .unwrap_or(story_id),
            story
                .frontmatter
                .get("sprint")
                .filter(|sprint| !sprint.trim().is_empty())
                .map(String::as_str)
                .unwrap_or("~"),
        );
        initial_markdown.as_str()
    };
    let updated = append_task_markdown(
        markdown,
        &task_id,
        title,
        &normalized_status,
        tags,
        description,
    );
    let task_file_path = task_file.file_path.clone();
    fs::write(&task_file_path, updated)
        .with_context(|| format!("write task file {}", task_file_path.display()))?;

    let task = Task {
        id: task_id.clone(),
        title: title.to_string(),
        status: display_task_status(&normalized_status).to_string(),
        normalized_status,
        tags: tags.to_vec(),
        description: description.to_string(),
    };

    Ok(TaskMutationResult {
        story_id: story.frontmatter.get("id").cloned().unwrap_or_default(),
        task_id,
        task_file_path: relative_path(&repository.repo_root, &task_file_path),
        task,
    })
}

pub fn update_task_in_story(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    task_id: &str,
    status: Option<&str>,
    title: Option<&str>,
    tags: Option<&[String]>,
    description: Option<&str>,
) -> Result<TaskMutationResult> {
    let repository = read_repository(repo_root)?;
    let story = find_story_for_write(&repository, story_id)?;
    let task_file = story
        .task_file
        .as_ref()
        .ok_or_else(|| anyhow!("Task file does not exist for story {}.", story_id))?;
    let markdown = task_file
        .markdown
        .as_deref()
        .ok_or_else(|| anyhow!("Task file does not exist for story {}.", story_id))?;
    let updated = rewrite_task_markdown(
        markdown,
        task_id,
        status
            .map(normalize_task_status_for_write)
            .transpose()?
            .as_deref(),
        title,
        tags,
        description,
    )?;
    let task_file_path = task_file.file_path.clone();
    fs::write(&task_file_path, updated.clone())
        .with_context(|| format!("write task file {}", task_file_path.display()))?;

    let normalized_task_id = task_id.trim().to_ascii_uppercase();
    let task = parse_task_markdown(&updated)
        .into_iter()
        .find(|t| t.id.eq_ignore_ascii_case(&normalized_task_id))
        .ok_or_else(|| anyhow!("Task {} not found after writing.", normalized_task_id))?;

    Ok(TaskMutationResult {
        story_id: story.frontmatter.get("id").cloned().unwrap_or_default(),
        task_id: normalized_task_id,
        task_file_path: relative_path(&repository.repo_root, &task_file_path),
        task,
    })
}

pub fn delete_task_from_story(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    task_id: &str,
) -> Result<TaskMutationResult> {
    let repository = read_repository(repo_root)?;
    let story = find_story_for_write(&repository, story_id)?;
    let task_file = story
        .task_file
        .as_ref()
        .ok_or_else(|| anyhow!("Task file does not exist for story {}.", story_id))?;
    let markdown = task_file
        .markdown
        .as_deref()
        .ok_or_else(|| anyhow!("Task file does not exist for story {}.", story_id))?;
    let normalized_task_id = task_id.trim().to_ascii_uppercase();
    let tasks = parse_task_markdown(markdown);
    let removed = tasks
        .iter()
        .find(|task| task.id.eq_ignore_ascii_case(&normalized_task_id))
        .cloned()
        .ok_or_else(|| anyhow!("Task not found: {normalized_task_id}"))?;
    let remaining = tasks
        .into_iter()
        .filter(|task| !task.id.eq_ignore_ascii_case(&normalized_task_id))
        .collect::<Vec<_>>();
    let story_id_value = story.frontmatter.get("id").cloned().unwrap_or_default();
    let sprint_name = story
        .frontmatter
        .get("sprint")
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "~".to_string());
    let updated = render_task_file(&story_id_value, &sprint_name, &remaining);
    let task_file_path = task_file.file_path.clone();
    fs::write(&task_file_path, updated)
        .with_context(|| format!("write task file {}", task_file_path.display()))?;

    Ok(TaskMutationResult {
        story_id: story_id_value,
        task_id: normalized_task_id,
        task_file_path: relative_path(&repository.repo_root, &task_file_path),
        task: removed,
    })
}

pub fn list_tasks_for_story(
    repo_root: impl AsRef<Path>,
    story_id: &str,
) -> Result<Option<TaskListResult>> {
    let repo_root = repo_root.as_ref();
    let details = find_story(repo_root, story_id)?;
    Ok(details.map(|details| TaskListResult {
        story_id: details.story.id,
        task_file_path: details.task_file_path,
        task_summary: details.story.task_summary,
        tasks: details.tasks,
    }))
}

pub fn story_markdown_file(repo_root: impl AsRef<Path>, story_id: &str) -> Result<StoryFileResult> {
    let repository = read_repository(repo_root)?;
    let story = find_story_for_write(&repository, story_id)?;
    Ok(StoryFileResult {
        story_id: story.frontmatter.get("id").cloned().unwrap_or_default(),
        story_path: story.relative_path.clone(),
        absolute_path: story.file_path.clone(),
    })
}

pub fn update_story_frontmatter(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    updates: &[(String, String)],
) -> Result<StoryUpdateResult> {
    let config = load_kanban_config(repo_root)?;
    let repository = read_repository(&config.repo_root)?;
    let story = find_story_for_write(&repository, story_id)?;
    if updates.is_empty() {
        bail!("No story frontmatter fields were provided.");
    }

    let normalized_updates = updates
        .iter()
        .map(|(field, value)| {
            let value = match field.as_str() {
                "assignee" => normalize_story_assignee_value(value)?,
                "sprint" => {
                    validate_story_sprint_frontmatter(value)?;
                    value.clone()
                }
                "priority" => {
                    validate_non_negative_integer_frontmatter(field, value)?;
                    value.clone()
                }
                _ => value.clone(),
            };
            Ok((field.clone(), value))
        })
        .collect::<Result<Vec<_>>>()?;

    let update_refs = normalized_updates
        .iter()
        .map(|(field, value)| (field.as_str(), Some(value.clone())))
        .collect::<Vec<_>>();
    let updated = upsert_frontmatter_markdown(&story.markdown, &update_refs)?;
    fs::write(&story.file_path, updated)
        .with_context(|| format!("write story file {}", story.file_path.display()))?;

    let mut affected_sprints = BTreeSet::new();
    if let Some(sprint) = story.frontmatter.get("sprint")
        && !sprint.trim().is_empty()
        && sprint.as_str() != "~"
    {
        affected_sprints.insert(sprint.clone());
    }
    if let Some((_, sprint)) = normalized_updates
        .iter()
        .find(|(field, _)| field == "sprint")
        && !sprint.trim().is_empty()
        && sprint.as_str() != "~"
    {
        affected_sprints.insert(sprint.clone());
    }
    for sprint in affected_sprints {
        regenerate_sprint_roster(&config, &sprint)?;
    }

    Ok(StoryUpdateResult {
        story_id: story.frontmatter.get("id").cloned().unwrap_or_default(),
        story_path: story.relative_path.clone(),
        updated_fields: normalized_updates
            .iter()
            .map(|(field, _)| field.clone())
            .collect(),
    })
}

pub fn find_story(repo_root: impl AsRef<Path>, story_id: &str) -> Result<Option<StoryDetails>> {
    let repo_root = repo_root.as_ref();
    let repository = read_repository(repo_root)?;
    Ok(find_story_in_repository(repo_root, &repository, story_id))
}

/// Find a story by id, returning both its [`StoryDetails`] and the raw parsed
/// [`Story`] (frontmatter + body) from a single repository scan.
pub fn find_story_with_source(
    repo_root: impl AsRef<Path>,
    story_id: &str,
) -> Result<Option<(StoryDetails, Story)>> {
    let repository = read_repository(repo_root)?;
    let normalized = story_id.trim().to_ascii_uppercase();
    // Use the same id-matching predicate as find_story_in_repository.
    let raw_story = {
        let mut matches: Vec<&Story> = repository
            .stories
            .iter()
            .filter(|s| {
                s.frontmatter
                    .get("id")
                    .map(|id| id.eq_ignore_ascii_case(&normalized))
                    .unwrap_or(false)
            })
            .collect();
        matches.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        matches.into_iter().next().cloned()
    };
    match raw_story {
        None => Ok(None),
        Some(raw_story) => {
            // Build details from the same repository (second pass over the same Vec).
            let details =
                find_story_in_repository(repository.repo_root.as_path(), &repository, story_id)
                    .expect("story was found in the same repository scan");
            Ok(Some((details, raw_story)))
        }
    }
}

pub(crate) fn unique_story_overviews(repository: &Repository) -> Vec<StoryOverview> {
    let mut selected = BTreeMap::<String, &Story>::new();

    for story in &repository.stories {
        let Some(id) = story.frontmatter.get("id") else {
            continue;
        };
        let normalized_id = id.trim().to_ascii_uppercase();
        if normalized_id.is_empty() {
            continue;
        }

        let replace_existing = selected
            .get(&normalized_id)
            .map(|existing| story.relative_path < existing.relative_path)
            .unwrap_or(true);
        if replace_existing {
            selected.insert(normalized_id, story);
        }
    }

    selected
        .into_values()
        .map(|story| story_overview(&repository.repo_root, story))
        .collect()
}

pub(crate) fn find_story_in_repository(
    repo_root: &Path,
    repository: &Repository,
    story_id: &str,
) -> Option<StoryDetails> {
    let normalized_story_id = story_id.trim().to_ascii_uppercase();
    let mut matches = repository
        .stories
        .iter()
        .filter(|story| {
            story
                .frontmatter
                .get("id")
                .map(|value| value.eq_ignore_ascii_case(&normalized_story_id))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    let story = matches.into_iter().next()?;
    Some(StoryDetails {
        story: story_overview(repo_root, story),
        story_file_path: story.relative_path.clone(),
        task_file_path: story
            .task_file
            .as_ref()
            .map(|task_file| task_file.relative_path.clone()),
        epic_id: story.frontmatter.get("epic").cloned(),
        epic_title: epic_title(repo_root, story),
        work_started: story.frontmatter.get("work_started").cloned(),
        work_done: story.frontmatter.get("work_done").cloned(),
        story_statement: extract_markdown_section(&story.body, "Story Statement"),
        acceptance_criteria: extract_markdown_section(&story.body, "Acceptance Criteria"),
        definition_of_done: extract_markdown_section(&story.body, "Definition of Done"),
        notes_and_open_questions: extract_markdown_section(&story.body, "Notes and Open Questions"),
        tasks: story
            .task_file
            .as_ref()
            .map(|task_file| task_file.tasks.clone())
            .unwrap_or_default(),
    })
}

pub(crate) fn normalize_story_status_input(status: &str) -> Result<String> {
    let normalized = normalize_status_alias(status);
    if CANONICAL_STORY_STATUSES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        bail!("Unsupported story status: {status}");
    }
}

pub(crate) fn validate_non_negative_integer_frontmatter(
    field_name: &str,
    value: &str,
) -> Result<()> {
    value
        .trim()
        .parse::<u32>()
        .map(|_| ())
        .map_err(|_| anyhow!("Frontmatter field \"{field_name}\" must be a non-negative integer."))
}

pub(crate) fn validate_story_sprint_frontmatter(value: &str) -> Result<()> {
    let sprint = value.trim();
    if sprint.is_empty() || sprint == "~" {
        return Ok(());
    }

    if parse_sprint_file_name(&format!("{sprint}.md")).is_some() {
        Ok(())
    } else {
        bail!(
            "Frontmatter field \"sprint\" must be empty, ~, or use <Snnn>.<headline-slug>; got {value:?}."
        );
    }
}

pub(crate) fn normalize_task_status_for_write(status: &str) -> Result<String> {
    let normalized = normalize_task_status(status);
    if CANONICAL_TASK_STATUSES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        bail!("Unsupported task status: {status}");
    }
}

pub(crate) fn find_story_for_write<'a>(
    repository: &'a Repository,
    story_id: &str,
) -> Result<&'a Story> {
    let normalized_story_id = story_id.trim().to_ascii_uppercase();
    repository
        .stories
        .iter()
        .find(|story| {
            story
                .frontmatter
                .get("id")
                .map(|id| id.eq_ignore_ascii_case(&normalized_story_id))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow!("Story not found: {normalized_story_id}"))
}

pub(crate) fn story_overview(repo_root: &Path, story: &Story) -> StoryOverview {
    StoryOverview {
        id: story.frontmatter.get("id").cloned().unwrap_or_else(|| {
            story
                .file_name
                .trim_end_matches(STORY_FILE_SUFFIX)
                .to_string()
        }),
        title: story_title(&story.body).unwrap_or_else(|| story.file_name.clone()),
        status: story.frontmatter.get("status").cloned().unwrap_or_default(),
        epic_id: story.frontmatter.get("epic").cloned(),
        epic_title: epic_title(repo_root, story),
        assignee: story
            .frontmatter
            .get("assignee")
            .cloned()
            .unwrap_or_default(),
        story_points: story
            .frontmatter
            .get("story_points")
            .cloned()
            .unwrap_or_default(),
        sprint: story.frontmatter.get("sprint").cloned(),
        relative_path: story.relative_path.clone(),
        task_summary: story
            .task_file
            .as_ref()
            .map(|task_file| task_file.summary.clone()),
        task_count: story
            .task_file
            .as_ref()
            .map(|task_file| task_file.tasks.len())
            .unwrap_or(0),
        work_started: story
            .frontmatter
            .get("work_started")
            .filter(|v| !v.trim().is_empty())
            .cloned(),
        work_done: story
            .frontmatter
            .get("work_done")
            .filter(|v| !v.trim().is_empty())
            .cloned(),
        planned_start: story
            .frontmatter
            .get("planned_start")
            .filter(|v| !v.trim().is_empty())
            .cloned(),
        planned_end: story
            .frontmatter
            .get("planned_end")
            .filter(|v| !v.trim().is_empty())
            .cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use tempfile::tempdir;

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
    fn normalize_story_status_input_treats_backlog_as_ready() {
        assert_eq!(normalize_story_status_input("backlog").unwrap(), "ready");
        assert_eq!(normalize_story_status_input("Backlog").unwrap(), "ready");
    }

    #[test]
    fn update_story_frontmatter_normalizes_multiple_assignees() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-098-test-story.md",
            "id: US-F1-098\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint:\nstory_points: 3\nassignee: TBD\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        update_story_frontmatter(
            temp_root.path(),
            "US-F1-098",
            &[(
                "assignee".to_string(),
                " Alice Example <alice@example.com> , Bob Berg <bob@example.com> ".to_string(),
            )],
        )
        .unwrap();

        let markdown = fs::read_to_string(story_path).unwrap();
        assert!(
            markdown.contains(
                "assignee: Alice Example <alice@example.com>, Bob Berg <bob@example.com>"
            )
        );
    }

    #[test]
    fn update_story_frontmatter_rejects_negative_priority() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-097-test-story.md",
            "id: US-F1-097\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let err = update_story_frontmatter(
            temp_root.path(),
            "US-F1-097",
            &[("priority".to_string(), "-1".to_string())],
        )
        .unwrap_err();

        assert!(err.to_string().contains("non-negative integer"));
        let markdown = fs::read_to_string(story_path).unwrap();
        assert!(!markdown.contains("priority:"));
    }

    #[test]
    fn update_story_frontmatter_rejects_invalid_sprint_value() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-096-test-story.md",
            "id: US-F1-096\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let err = update_story_frontmatter(
            temp_root.path(),
            "US-F1-096",
            &[("sprint".to_string(), "/Users/tm".to_string())],
        )
        .unwrap_err();

        assert!(err.to_string().contains("<Snnn>.<headline-slug>"));
        let markdown = fs::read_to_string(story_path).unwrap();
        assert!(markdown.contains("sprint: ~"));
        assert!(!markdown.contains("sprint: /Users/tm"));
    }

    #[test]
    fn list_all_stories_returns_single_story_entry() {
        let (_fixture, repo_root) = build_fixture();

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
    fn find_story_exposes_acceptance_criteria_and_tasks() {
        let (_fixture, repo_root) = build_fixture();
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
    fn move_story_to_status_preserves_existing_assignee_and_updates_roster() {
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
        assert!(moved_story.contains("assignee: Old Owner <old@example.com>"));
        assert!(moved_story.contains("work_started: 20"));
        let sprint_markdown =
            fs::read_to_string(temp_root.path().join("delivery/sprints/S001.foundation.md"))
                .unwrap();
        assert!(sprint_markdown.contains("| Metric | Stories | Points |"));
        assert!(sprint_markdown.contains("| Story | Points | Assignee | Tasks |"));
        assert!(sprint_markdown.contains("### in-progress"));
        assert!(sprint_markdown.contains("mailto:old@example.com"));
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
    fn move_story_to_in_progress_preserves_existing_assignee_when_already_in_progress() {
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
        assert!(backlog_story.contains("assignee: Old Owner <old@example.com>"));
    }

    #[test]
    fn move_story_to_in_progress_sets_git_assignee_when_assignee_is_tbd() {
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
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
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
    fn move_story_to_in_progress_accepts_multiple_assignees_override() {
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

        move_story_to_status_with_assignee(
            temp_root.path(),
            "US-F1-053",
            "in-progress",
            Some("Override User <override@example.com>, Pair User <pair@example.com>"),
        )
        .unwrap();

        let backlog_story = fs::read_to_string(&story_path).unwrap();
        assert!(backlog_story.contains(
            "assignee: Override User <override@example.com>, Pair User <pair@example.com>"
        ));
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
        assert!(sprint_markdown.contains("| Metric | Stories | Points |"));
        assert!(sprint_markdown.contains("| Story | Points | Assignee | Tasks |"));
        assert!(sprint_markdown.contains("### todo"));
        assert!(sprint_markdown.contains("[**US-F2-001** Ingest passage events](../backlog/"));
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
            "# Tasks for US-F1-053\n\nParent User Story: US-F1-053\nSprint: S001.foundation\n",
        )
        .unwrap();

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
        assert!(updated_markdown.contains("Status: done"));
        assert!(updated_markdown.contains("Completed command coverage."));
    }

    #[test]
    fn task_add_does_not_duplicate_separator_when_appending() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-057-complete-kanban-cli-task-crud-for-story-task-logs.md",
            "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n",
        );

        add_task_to_story(
            temp_root.path(),
            "US-F1-057",
            "First task",
            "todo",
            &[],
            "First.",
        )
        .unwrap();

        add_task_to_story(
            temp_root.path(),
            "US-F1-057",
            "Second task",
            "todo",
            &[],
            "Second.",
        )
        .unwrap();

        let task_markdown = fs::read_to_string(story_path.with_extension("tasks.md")).unwrap();
        assert!(!task_markdown.contains("\n\n---\n\n---\n\n"));
        assert!(task_markdown.contains("## TASK-US-F1-057-001 - First task"));
        assert!(task_markdown.contains("## TASK-US-F1-057-002 - Second task"));
    }

    #[test]
    fn task_add_creates_sibling_task_file_for_backlog_story() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-057-complete-kanban-cli-task-crud-for-story-task-logs.md",
            "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n",
        );

        let add_result = add_task_to_story(
            temp_root.path(),
            "US-F1-057",
            "Plan taskable backlog stories",
            "todo",
            &["cli".to_string()],
            "Add task planning before sprint assignment.",
        )
        .unwrap();

        assert_eq!(add_result.story_id, "US-F1-057");
        assert_eq!(add_result.task_id, "TASK-US-F1-057-001");
        assert_eq!(
            temp_root.path().join(&add_result.task_file_path),
            story_path.with_extension("tasks.md")
        );
        let task_markdown = fs::read_to_string(story_path.with_extension("tasks.md")).unwrap();
        assert!(task_markdown.contains("# Tasks for US-F1-057"));
        assert!(task_markdown.contains("Sprint: ~"));
        assert!(task_markdown.contains("## TASK-US-F1-057-001 - Plan taskable backlog stories"));
    }

    #[test]
    fn story_delete_removes_story_task_file_and_updates_sprint_roster() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "Foundation",
            "2026-06-01",
            "2026-06-12",
            "active",
        );
        let story_path = write_story_with_task_file(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-057-complete-kanban-cli-task-crud-for-story-task-logs.md",
            "id: US-F1-057\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n",
        );

        regenerate_sprint_roster(
            &load_kanban_config(temp_root.path()).unwrap(),
            "S001.foundation",
        )
        .unwrap();
        let result = delete_story(temp_root.path(), "US-F1-057").unwrap();

        assert_eq!(result.story_id, "US-F1-057");
        assert_eq!(result.sprint_name.as_deref(), Some("S001.foundation"));
        assert_eq!(temp_root.path().join(&result.story_path), story_path);
        assert!(!story_path.exists());
        assert!(!story_path.with_extension("tasks.md").exists());
        let sprint_markdown =
            fs::read_to_string(temp_root.path().join("delivery/sprints/S001.foundation.md"))
                .unwrap();
        assert!(!sprint_markdown.contains("US-F1-057"));
    }

    #[test]
    fn task_delete_removes_matching_task_and_keeps_remaining_tasks() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-057-complete-kanban-cli-task-crud-for-story-task-logs.md",
            "id: US-F1-057\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nassignee: TBD\nstory_points: 1\nwork_started:\nwork_done:\ncreated: 2026-06-09T10:18:05+0200\nupdated: 2026-06-09T10:18:05+0200\n",
        );

        add_task_to_story(
            temp_root.path(),
            "US-F1-057",
            "First task",
            "todo",
            &[],
            "First.",
        )
        .unwrap();
        add_task_to_story(
            temp_root.path(),
            "US-F1-057",
            "Second task",
            "todo",
            &[],
            "Second.",
        )
        .unwrap();

        let removed =
            delete_task_from_story(temp_root.path(), "US-F1-057", "TASK-US-F1-057-001").unwrap();
        let task_markdown = fs::read_to_string(story_path.with_extension("tasks.md")).unwrap();

        assert_eq!(removed.task_id, "TASK-US-F1-057-001");
        assert!(!task_markdown.contains("## TASK-US-F1-057-001 - First task"));
        assert!(task_markdown.contains("## TASK-US-F1-057-002 - Second task"));
    }
}
