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
    let repository = read_repository(repo_root)?;
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

    let sprint_name = story
        .frontmatter
        .get("sprint")
        .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
        .cloned()
        .ok_or_else(|| anyhow!("Story {normalized_story_id} is not assigned to a sprint."))?;
    let current_status = story.frontmatter.get("status").cloned().unwrap_or_default();

    let assignee_update = if normalized_status == "in-progress" {
        Some(match assignee_override {
            Some(assignee) => assignee,
            None => current_git_assignee(&repository.repo_root)?,
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
    regenerate_sprint_roster(&load_kanban_config(&repository.repo_root)?, &sprint_name)?;

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
    let new_status = if matches!(current_status, "" | "draft" | "ready") {
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

pub fn add_task_to_story(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    title: &str,
    status: &str,
    tags: &[String],
    description: &str,
) -> Result<TaskMutationResult> {
    let repository = read_repository(repo_root)?;
    let story = find_sprint_story_for_write(&repository, story_id)?;
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
    let markdown = task_file.markdown.as_deref().unwrap_or_default();
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
    let story = find_sprint_story_for_write(&repository, story_id)?;
    let task_file = story
        .task_file
        .as_ref()
        .ok_or_else(|| anyhow!("Sprint story is missing task_file frontmatter."))?;
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

    let update_refs = updates
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
    if let Some((_, sprint)) = updates.iter().find(|(field, _)| field == "sprint")
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
        updated_fields: updates.iter().map(|(field, _)| field.clone()).collect(),
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
    let lowercase = status.trim().to_ascii_lowercase();
    let normalized = match lowercase.as_str() {
        "to do" => "todo",
        "in progress" => "in-progress",
        other => other,
    };
    if CANONICAL_STORY_STATUSES.contains(&normalized) {
        Ok(normalized.to_string())
    } else {
        bail!("Unsupported story status: {status}");
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

pub(crate) fn find_sprint_story_for_write<'a>(
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
                && story
                    .frontmatter
                    .get("sprint")
                    .is_some_and(|sprint| !sprint.trim().is_empty() && sprint.as_str() != "~")
        })
        .ok_or_else(|| anyhow!("Sprint story not found: {normalized_story_id}"))
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
    }
}
