use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Datelike, Days, Local, NaiveDate, TimeZone, Weekday};
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

mod config;

pub use config::{
    ColorMode, ConfigInitResult, ConfigSetResult, KanbanConfig, get_config_json, get_config_value,
    init_config, load_kanban_config, resolve_repo_root, set_config_value,
};

const REQUIRED_STORY_FIELDS: [&str; 10] = [
    "id",
    "type",
    "status",
    "epic",
    "sprint",
    "story_points",
    "work_started",
    "work_done",
    "created",
    "updated",
];

const CANONICAL_STORY_STATUSES: [&str; 8] = [
    "draft",
    "ready",
    "todo",
    "in-progress",
    "ready-for-qa",
    "blocked",
    "done",
    "dropped",
];
const TASK_HEADING_PATTERN: &str = r"(?m)^##\s+(TASK-[A-Z0-9-]+)\s+-\s+(.+)$";
const STORY_FILE_PREFIX: &str = "US-";
const EPIC_FILE_PREFIX: &str = "EP-";
const STORY_FILE_SUFFIX: &str = ".md";
const TASK_FILE_SUFFIX: &str = ".tasks.md";
const SPRINT_FILE_PATTERN: &str = r"^(S\d{3})\.([a-z0-9][a-z0-9-]*)\.md$";
const REQUIRED_SPRINT_README_FIELDS: [&str; 6] = [
    "sprint",
    "headline",
    "start_date",
    "end_date",
    "status",
    "wip_limit",
];
const SPRINT_STATUS_DISPLAY_ORDER: [&str; 5] =
    ["todo", "in-progress", "ready-for-qa", "done", "blocked"];
const STATUS_PROGRESSION: [&str; 6] = [
    "draft",
    "ready",
    "todo",
    "in-progress",
    "ready-for-qa",
    "done",
];
const SPRINT_STATUSES: [&str; 4] = ["planned", "active", "closed", "cancelled"];
const ROSTER_COLUMN_ORDER: [&str; 5] = ["in-progress", "ready-for-qa", "todo", "blocked", "done"];
const ROSTER_HEADING: &str = "## Stories (generated — do not edit)";
const CANONICAL_TASK_STATUSES: [&str; 4] = ["todo", "in-progress", "blocked", "done"];

fn status_rank(status: &str) -> Option<usize> {
    STATUS_PROGRESSION.iter().position(|s| *s == status)
}

pub fn most_advanced_status(statuses: &[&str]) -> String {
    let best_progression = statuses
        .iter()
        .filter_map(|s| status_rank(s).map(|rank| (rank, *s)))
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, status)| status.to_string());
    best_progression
        .or_else(|| statuses.first().map(|status| status.to_string()))
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFrontmatter {
    pub frontmatter: BTreeMap<String, String>,
    pub frontmatter_keys: BTreeSet<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
    pub normalized_status: String,
    pub tags: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskSummary {
    pub todo: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFile {
    pub exists: bool,
    pub file_path: PathBuf,
    pub relative_path: PathBuf,
    pub tasks: Vec<Task>,
    pub summary: TaskSummary,
    pub markdown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Story {
    pub file_path: PathBuf,
    pub relative_path: PathBuf,
    pub file_name: String,
    pub frontmatter: BTreeMap<String, String>,
    pub frontmatter_keys: BTreeSet<String>,
    pub markdown: String,
    pub body: String,
    pub sprint_name: Option<String>,
    pub task_file: Option<TaskFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub file_path: PathBuf,
    pub rule: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub repo_root: PathBuf,
    pub stories: Vec<Story>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub repo_root: PathBuf,
    pub stories: Vec<Story>,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryOverview {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assignee: String,
    pub story_points: String,
    pub sprint: Option<String>,
    pub relative_path: PathBuf,
    pub task_summary: Option<TaskSummary>,
    pub task_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedWorkItem {
    pub story_id: String,
    pub story_title: String,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SprintOverview {
    pub sprint_name: String,
    pub headline: String,
    pub sprint_goal: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub readme_path: PathBuf,
    pub readme_status: Option<String>,
    pub stories_by_status: BTreeMap<String, Vec<StoryOverview>>,
    pub blocked_work: Vec<BlockedWorkItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSprintInput {
    pub number: u32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub headline: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSprintResult {
    pub sprint_name: String,
    pub sprint_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveStoryResult {
    pub story_id: String,
    pub sprint_name: String,
    pub from_status: String,
    pub to_status: String,
    pub story_path: PathBuf,
    pub task_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStoryResult {
    pub story_id: String,
    pub sprint_name: String,
    pub story_path: PathBuf,
    pub task_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMutationResult {
    pub story_id: String,
    pub task_id: String,
    pub task_file_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryFileResult {
    pub story_id: String,
    pub story_path: PathBuf,
    pub absolute_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryUpdateResult {
    pub story_id: String,
    pub story_path: PathBuf,
    pub updated_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloverResult {
    pub from_sprint: String,
    pub to_sprint: String,
    pub created_next_sprint: bool,
    pub completed_story_ids: Vec<String>,
    pub carried_story_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseOverview {
    pub phase: String,
    pub stories: Vec<StoryOverview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub value: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryDetails {
    pub story: StoryOverview,
    pub task_file_path: Option<PathBuf>,
    pub story_statement: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub definition_of_done: Option<String>,
    pub notes_and_open_questions: Option<String>,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorFinding {
    pub severity: String,
    pub scope: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorFixKind {
    Automatic,
    Guided,
    ManualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorPrompt {
    None,
    Text {
        label: String,
        default: Option<String>,
    },
    Choice {
        label: String,
        options: Vec<String>,
        default: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorFixPreview {
    pub field_name: String,
    pub old_value: String,
    pub new_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorIssue {
    pub severity: String,
    pub scope: String,
    pub file_path: Option<PathBuf>,
    pub story_id: Option<String>,
    pub sprint_name: Option<String>,
    pub rule: String,
    pub message: String,
    pub suggestion: String,
    pub fix_preview: Option<DoctorFixPreview>,
    pub fix_kind: DoctorFixKind,
    pub prompt: DoctorPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DoctorFixInput {
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorFixResult {
    pub message: String,
    pub touched_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SprintFolderSpec {
    sprint_name: String,
    headline: String,
    sprint_goal: Option<String>,
    start_date: NaiveDate,
    end_date: NaiveDate,
    readme_path: PathBuf,
    readme_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SprintReadmeInfo {
    sprint: Option<String>,
    headline: Option<String>,
    sprint_goal: Option<String>,
    status: Option<String>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
}

pub fn parse_frontmatter(markdown: &str) -> ParsedFrontmatter {
    let normalized = markdown.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") {
        return ParsedFrontmatter {
            frontmatter: BTreeMap::new(),
            frontmatter_keys: BTreeSet::new(),
            body: normalized,
        };
    }

    let lines: Vec<&str> = normalized.split('\n').collect();
    let closing_index = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index));

    let Some(closing_index) = closing_index else {
        return ParsedFrontmatter {
            frontmatter: BTreeMap::new(),
            frontmatter_keys: BTreeSet::new(),
            body: normalized,
        };
    };

    let mut frontmatter = BTreeMap::new();
    let mut frontmatter_keys = BTreeSet::new();

    for line in &lines[1..closing_index] {
        if line.trim().is_empty() {
            continue;
        }

        let Some((key, raw_value)) = line.split_once(':') else {
            continue;
        };

        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }

        frontmatter_keys.insert(key.to_string());
        frontmatter.insert(key.to_string(), parse_scalar(raw_value));
    }

    ParsedFrontmatter {
        frontmatter,
        frontmatter_keys,
        body: lines[(closing_index + 1)..].join("\n"),
    }
}

pub fn parse_task_markdown(markdown: &str) -> Vec<Task> {
    let normalized = markdown.replace("\r\n", "\n");
    let heading_pattern = Regex::new(TASK_HEADING_PATTERN).expect("valid task heading regex");
    let matches: Vec<_> = heading_pattern
        .captures_iter(&normalized)
        .filter_map(|captures| {
            let full = captures.get(0)?;
            let id = captures.get(1)?.as_str().to_string();
            let title = captures.get(2)?.as_str().trim().to_string();
            Some((full.start(), full.end(), id, title))
        })
        .collect();

    let mut tasks = Vec::new();
    for (index, (_, block_start, id, title)) in matches.iter().enumerate() {
        let block_end = matches
            .get(index + 1)
            .map(|next| next.0)
            .unwrap_or(normalized.len());
        let block = &normalized[*block_start..block_end];
        let status = capture_line_value(block, "Status").unwrap_or_default();
        let tags_value = capture_line_value(block, "Tags").unwrap_or_default();
        let description = capture_description(block);

        tasks.push(Task {
            id: id.clone(),
            title: title.clone(),
            normalized_status: normalize_task_status(&status),
            status,
            tags: tags_value
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            description,
        });
    }

    tasks
}

pub fn create_task_summary(tasks: &[Task]) -> TaskSummary {
    let mut summary = TaskSummary::default();
    for task in tasks {
        match task.normalized_status.as_str() {
            "todo" => summary.todo += 1,
            "in-progress" => summary.in_progress += 1,
            "blocked" => summary.blocked += 1,
            "done" => summary.done += 1,
            _ => {}
        }
    }
    summary
}

pub fn collect_user_story_files(repo_root: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let config = load_kanban_config(repo_root)?;
    let backlog_root = config.backlog_path();
    let mut files = Vec::new();

    for entry in WalkDir::new(&backlog_root)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if name.starts_with(STORY_FILE_PREFIX)
            && name.ends_with(STORY_FILE_SUFFIX)
            && !name.ends_with(TASK_FILE_SUFFIX)
        {
            files.push(entry.into_path());
        }
    }

    files.sort();
    Ok(files)
}

/// Collect all epic markdown files (`EP-*.md`) from the backlog tree.
pub fn collect_epic_files(repo_root: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let config = load_kanban_config(repo_root)?;
    let backlog_root = config.backlog_path();
    let mut files = Vec::new();

    for entry in WalkDir::new(&backlog_root)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if name.starts_with(EPIC_FILE_PREFIX) && name.ends_with(STORY_FILE_SUFFIX) {
            files.push(entry.into_path());
        }
    }

    files.sort();
    Ok(files)
}

/// Return all sprint folder names (e.g. `S000.getting-started`) sorted alphabetically.
/// This is a lightweight listing suitable for shell completion.
pub fn list_sprint_names(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let config = load_kanban_config(repo_root)?;
    let mut specs = discover_sprint_folder_specs(&config)?;
    specs.sort_by(|a, b| a.sprint_name.cmp(&b.sprint_name));
    Ok(specs.into_iter().map(|s| s.sprint_name).collect())
}

/// Return unique user story IDs (e.g. `US-F1-053`) sorted alphabetically.
/// Each ID appears only once regardless of how many copies (sprint vs backlog) exist.
/// This is a lightweight listing suitable for shell completion.
pub fn list_story_ids(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let repository = read_repository(repo_root)?;
    let mut seen = BTreeSet::new();
    for story in &repository.stories {
        if let Some(id) = story.frontmatter.get("id") {
            let id_upper = id.trim().to_ascii_uppercase();
            if !id_upper.is_empty() {
                seen.insert(id_upper);
            }
        }
    }
    Ok(seen.into_iter().collect())
}

/// Return user story completion values with display descriptions.
/// `value` is the inserted shell completion; `description` is menu text only.
pub fn list_story_completion_items(repo_root: impl AsRef<Path>) -> Result<Vec<CompletionItem>> {
    let repository = read_repository(repo_root)?;
    let mut items = BTreeMap::new();
    for story in &repository.stories {
        if let Some(id) = story.frontmatter.get("id") {
            let id_upper = id.trim().to_ascii_uppercase();
            if !id_upper.is_empty() {
                let title = story_title(&story.body).unwrap_or_else(|| story.file_name.clone());
                items.entry(id_upper).or_insert(title);
            }
        }
    }

    Ok(items
        .into_iter()
        .map(|(value, description)| CompletionItem { value, description })
        .collect())
}

/// Return epic IDs (e.g. `EP-F1-06`) from all `EP-*.md` files in the backlog.
/// IDs are read from frontmatter `id` field; missing/empty entries are skipped.
/// This is a lightweight listing suitable for shell completion.
pub fn list_epic_ids(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let files = collect_epic_files(repo_root)?;
    let mut ids = BTreeSet::new();
    for file in &files {
        if let Ok(markdown) = fs::read_to_string(file) {
            let parsed = parse_frontmatter(&markdown);
            if let Some(id) = parsed.frontmatter.get("id") {
                let id_upper = id.trim().to_ascii_uppercase();
                if !id_upper.is_empty() {
                    ids.insert(id_upper);
                }
            }
        }
    }
    Ok(ids.into_iter().collect())
}

pub fn read_task_file(
    file_path: impl AsRef<Path>,
    repo_root: impl AsRef<Path>,
) -> Result<TaskFile> {
    let file_path = fs::canonicalize(file_path.as_ref())
        .with_context(|| format!("resolve task file {}", file_path.as_ref().display()))?;
    let markdown = fs::read_to_string(&file_path)
        .with_context(|| format!("read task file {}", file_path.display()))?;
    let tasks = parse_task_markdown(&markdown);
    Ok(TaskFile {
        exists: true,
        relative_path: relative_path(repo_root.as_ref(), &file_path),
        summary: create_task_summary(&tasks),
        tasks,
        markdown: Some(markdown),
        file_path,
    })
}

pub fn read_story_file(file_path: impl AsRef<Path>, repo_root: impl AsRef<Path>) -> Result<Story> {
    let repo_root = repo_root.as_ref();
    let config = load_kanban_config(repo_root)?;
    let file_path = fs::canonicalize(file_path.as_ref())
        .with_context(|| format!("resolve story file {}", file_path.as_ref().display()))?;
    let markdown = fs::read_to_string(&file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    let location = story_location(&file_path, &config);
    let sprint_name = parsed
        .frontmatter
        .get("sprint")
        .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
        .cloned()
        .or(location.sprint_name.clone());
    let sibling_task_file_path = file_path.with_extension("tasks.md");
    let referenced_task_file_path = parsed
        .frontmatter
        .get("task_file")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| file_path.parent().unwrap().join(value));
    let task_file_path = referenced_task_file_path
        .clone()
        .unwrap_or_else(|| sibling_task_file_path.clone());
    let task_file = if task_file_path.exists() {
        Some(read_task_file(&task_file_path, repo_root)?)
    } else if referenced_task_file_path.is_some() {
        Some(TaskFile {
            exists: false,
            file_path: task_file_path.clone(),
            relative_path: relative_path(repo_root, &task_file_path),
            tasks: Vec::new(),
            summary: TaskSummary::default(),
            markdown: None,
        })
    } else {
        None
    };

    Ok(Story {
        relative_path: relative_path(repo_root, &file_path),
        file_name: file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        body: parsed.body,
        file_path,
        frontmatter: parsed.frontmatter,
        frontmatter_keys: parsed.frontmatter_keys,
        markdown,
        sprint_name,
        task_file,
    })
}

pub fn read_repository(repo_root: impl AsRef<Path>) -> Result<Repository> {
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root.clone();
    let story_files = collect_user_story_files(&repo_root)?;
    let stories = story_files
        .into_iter()
        .map(|path| read_story_file(path, &repo_root))
        .collect::<Result<Vec<_>>>()?;
    Ok(Repository { repo_root, stories })
}

pub fn summarize_sprints(repo_root: impl AsRef<Path>) -> Result<Vec<SprintOverview>> {
    let repository = read_repository(repo_root)?;
    summarize_sprints_from_repository(&repository)
}

pub fn summarize_current_sprint(repo_root: impl AsRef<Path>) -> Result<SprintOverview> {
    summarize_current_sprint_at_date(repo_root, Local::now().date_naive())
}

pub fn summarize_current_sprint_at_date(
    repo_root: impl AsRef<Path>,
    today: NaiveDate,
) -> Result<SprintOverview> {
    let repository = read_repository(repo_root)?;
    let sprints = summarize_sprints_from_repository(&repository)?;
    select_current_sprint(&sprints, today)
}

pub fn summarize_sprint(repo_root: impl AsRef<Path>, sprint_name: &str) -> Result<SprintOverview> {
    let repository = read_repository(repo_root)?;
    let sprints = summarize_sprints_from_repository(&repository)?;
    sprints
        .into_iter()
        .find(|sprint| sprint.sprint_name == sprint_name)
        .ok_or_else(|| anyhow!("Sprint not found: {sprint_name}"))
}

pub fn summarize_phase(repo_root: impl AsRef<Path>, phase: &str) -> Result<PhaseOverview> {
    let repository = read_repository(repo_root)?;
    let phase_number = normalize_phase_input(phase)?;
    let config = load_kanban_config(&repository.repo_root)?;
    let phase_marker = format!("{}phase-{phase_number}-", config.backlog_marker());
    let mut stories = repository
        .stories
        .iter()
        .filter(|story| to_forward_slashes(&story.file_path).contains(&phase_marker))
        .map(story_overview)
        .collect::<Vec<_>>();

    stories.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(PhaseOverview {
        phase: format!("F{phase_number}"),
        stories,
    })
}

pub fn list_all_stories(repo_root: impl AsRef<Path>) -> Result<Vec<StoryOverview>> {
    let repository = read_repository(repo_root)?;
    Ok(unique_story_overviews(&repository))
}

pub fn list_current_sprint_stories(
    repo_root: impl AsRef<Path>,
) -> Result<(String, Vec<StoryOverview>)> {
    let sprint = summarize_current_sprint(repo_root)?;
    let sprint_name = sprint.sprint_name.clone();
    Ok((sprint_name, flatten_sprint_stories(&sprint)))
}

pub fn list_next_sprint_stories(
    repo_root: impl AsRef<Path>,
) -> Result<(String, Vec<StoryOverview>)> {
    let repository = read_repository(repo_root)?;
    let sprints = summarize_sprints_from_repository(&repository)?;
    let current = select_current_sprint(&sprints, Local::now().date_naive())?;
    let current_number = parse_sprint_number(&current.sprint_name).ok_or_else(|| {
        anyhow!(
            "Current sprint name does not use the expected SNNN.headline format: {}",
            current.sprint_name
        )
    })?;

    let next = sprints
        .into_iter()
        .filter_map(|sprint| {
            parse_sprint_number(&sprint.sprint_name)
                .filter(|number| *number > current_number)
                .map(|number| (number, sprint))
        })
        .min_by_key(|(number, _)| *number)
        .map(|(_, sprint)| sprint)
        .ok_or_else(|| anyhow!("No later sprint exists after {}.", current.sprint_name))?;

    let sprint_name = next.sprint_name.clone();
    Ok((sprint_name, flatten_sprint_stories(&next)))
}

pub fn list_stories_in_sprint(
    repo_root: impl AsRef<Path>,
    sprint_name: &str,
) -> Result<Vec<StoryOverview>> {
    let sprint = summarize_sprint(repo_root, sprint_name)?;
    Ok(flatten_sprint_stories(&sprint))
}

pub fn suggested_next_sprint_number(repo_root: impl AsRef<Path>) -> Result<u32> {
    let config = load_kanban_config(repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    Ok(specs
        .iter()
        .filter_map(|spec| parse_sprint_number(&spec.sprint_name))
        .max()
        .map(|value| value + 1)
        .unwrap_or(0))
}

pub fn suggested_next_sprint_dates(
    repo_root: impl AsRef<Path>,
) -> Result<Option<(NaiveDate, NaiveDate)>> {
    let config = load_kanban_config(repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    let previous_end_date = specs
        .iter()
        .filter_map(|spec| {
            parse_sprint_number(&spec.sprint_name).map(|number| (number, spec.end_date))
        })
        .max_by_key(|(number, _)| *number)
        .map(|(_, end_date)| end_date);

    Ok(previous_end_date.map(suggested_sprint_dates))
}

pub fn suggested_sprint_dates(previous_end_date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start_date = first_weekday_after(previous_end_date, Weekday::Mon);
    let end_date = first_weekday_on_or_after(start_date + Days::new(11), Weekday::Fri);
    (start_date, end_date)
}

pub fn create_sprint(
    repo_root: impl AsRef<Path>,
    input: &CreateSprintInput,
) -> Result<CreateSprintResult> {
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root.clone();
    let today = Local::now().date_naive();
    if input.start_date < today {
        bail!(
            "Sprint start date {} cannot be in the past relative to {}.",
            input.start_date.format("%Y-%m-%d"),
            today.format("%Y-%m-%d")
        );
    }
    if input.end_date <= input.start_date {
        bail!(
            "Sprint end date {} must be after start date {}.",
            input.end_date.format("%Y-%m-%d"),
            input.start_date.format("%Y-%m-%d")
        );
    }

    let headline = slugify_headline(&input.headline);
    if headline.is_empty() {
        bail!("Sprint headline must contain at least one ASCII letter or number.");
    }

    let sprint_id = format!("S{:03}", input.number);
    let sprint_name = format!("{sprint_id}.{headline}");
    let sprint_file = config.sprints_path().join(format!("{sprint_name}.md"));
    if sprint_file.exists() {
        bail!("Sprint already exists: {sprint_name}");
    }

    fs::create_dir_all(config.sprints_path())
        .with_context(|| format!("create sprints dir {}", config.sprints_path().display()))?;
    let content =
        render_sprint_file_template(&sprint_id, &headline, input.start_date, input.end_date);
    fs::write(&sprint_file, content)
        .with_context(|| format!("write sprint file {}", sprint_file.display()))?;

    Ok(CreateSprintResult {
        sprint_name,
        sprint_path: relative_path(&repo_root, &sprint_file),
    })
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

    Ok(TaskMutationResult {
        story_id: story.frontmatter.get("id").cloned().unwrap_or_default(),
        task_id,
        task_file_path: relative_path(&repository.repo_root, &task_file_path),
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
    fs::write(&task_file_path, updated)
        .with_context(|| format!("write task file {}", task_file_path.display()))?;

    Ok(TaskMutationResult {
        story_id: story.frontmatter.get("id").cloned().unwrap_or_default(),
        task_id: task_id.trim().to_ascii_uppercase(),
        task_file_path: relative_path(&repository.repo_root, &task_file_path),
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

pub fn rollover_sprint(
    repo_root: impl AsRef<Path>,
    sprint_name: &str,
    next_sprint: Option<&CreateSprintInput>,
) -> Result<RolloverResult> {
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root.clone();
    let repository = read_repository(&repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    let current_spec = specs
        .iter()
        .find(|spec| spec.sprint_name == sprint_name)
        .ok_or_else(|| anyhow!("Sprint not found: {sprint_name}"))?;

    let expected_next_number = parse_sprint_number(sprint_name).map(|value| value + 1);
    let mut next_sprint_name = specs
        .iter()
        .find(|spec| parse_sprint_number(&spec.sprint_name) == expected_next_number)
        .map(|spec| spec.sprint_name.clone());
    let mut created_next_sprint = false;

    if next_sprint_name.is_none() {
        let input = next_sprint.ok_or_else(|| {
            anyhow!(
                "Next sprint is missing after {sprint_name}. Create it first or provide create input."
            )
        })?;
        let create_result = create_sprint(&repo_root, input)?;
        created_next_sprint = true;
        next_sprint_name = Some(create_result.sprint_name);
    }

    let next_sprint_name = next_sprint_name.ok_or_else(|| anyhow!("Next sprint is missing."))?;
    let mut completed_story_ids = Vec::new();
    let mut carried_story_ids = Vec::new();

    for story in repository
        .stories
        .iter()
        .filter(|story| story.frontmatter.get("sprint").map(String::as_str) == Some(sprint_name))
    {
        let story_id = story.frontmatter.get("id").cloned().unwrap_or_default();
        let status = story.frontmatter.get("status").cloned().unwrap_or_default();
        if status == "done" {
            completed_story_ids.push(story_id);
            continue;
        }

        let now = current_timestamp_string();
        let moved_story_markdown = update_story_frontmatter_markdown(
            &story.markdown,
            &[
                ("sprint", Some(next_sprint_name.clone())),
                ("updated", Some(now.clone())),
                (
                    "work_started",
                    story.frontmatter.get("work_started").cloned(),
                ),
            ],
        )?;
        fs::write(&story.file_path, moved_story_markdown).with_context(|| {
            format!("rewrite rolled sprint story {}", story.file_path.display())
        })?;

        carried_story_ids.push(story_id);
    }

    let closed_readme_path = current_spec.readme_path.clone();
    let closed_readme = fs::read_to_string(&closed_readme_path)
        .with_context(|| format!("read sprint summary {}", closed_readme_path.display()))?;
    let closed_readme = update_sprint_summary_for_rollover(
        &closed_readme,
        sprint_name,
        &next_sprint_name,
        &completed_story_ids,
        &carried_story_ids,
    );
    fs::write(&closed_readme_path, closed_readme)
        .with_context(|| format!("write sprint summary {}", closed_readme_path.display()))?;
    regenerate_sprint_roster(&config, sprint_name)?;
    regenerate_sprint_roster(&config, &next_sprint_name)?;

    Ok(RolloverResult {
        from_sprint: sprint_name.to_string(),
        to_sprint: next_sprint_name,
        created_next_sprint,
        completed_story_ids,
        carried_story_ids,
    })
}

pub fn find_story(repo_root: impl AsRef<Path>, story_id: &str) -> Result<Option<StoryDetails>> {
    let repository = read_repository(repo_root)?;
    Ok(find_story_in_repository(&repository, story_id))
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

fn collect_doctor_issues_at_date(
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

fn doctor_issue_from_validation(
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

fn doctor_fix_preview_for_validation(
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

fn frontmatter_fix_preview(
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

fn date_only_timestamp_issue(story: Option<&Story>, issue: &ValidationIssue) -> bool {
    let Some(field_name) = issue.rule.strip_prefix("invalid-timestamp:") else {
        return false;
    };
    story
        .and_then(|story| story.frontmatter.get(field_name))
        .and_then(|value| date_only_timestamp(value))
        .is_some()
}

fn doctor_suggestion_for_validation(
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

    validate_timestamp_field(story, &mut issues, "created", false);
    validate_timestamp_field(story, &mut issues, "updated", false);
    validate_timestamp_field(story, &mut issues, "work_started", true);
    validate_timestamp_field(story, &mut issues, "work_done", true);

    if story
        .frontmatter
        .get("sprint")
        .is_some_and(|sprint| !sprint.trim().is_empty() && sprint.as_str() != "~")
    {
        validate_timestamp_field(story, &mut issues, "activated", true);
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

    if assignee_required(story) && !story.frontmatter_keys.contains("assignee") {
        add_issue(
            story,
            &mut issues,
            "missing-field:assignee",
            "Stories with started work must have assignee set.".to_string(),
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

fn summarize_sprints_from_repository(repository: &Repository) -> Result<Vec<SprintOverview>> {
    let today = Local::now().date_naive();
    let config = load_kanban_config(&repository.repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    let mut sprints = specs
        .iter()
        .map(|spec| sprint_overview_from_spec(repository, spec, today))
        .collect::<Vec<_>>();
    sprints.sort_by(|left, right| left.sprint_name.cmp(&right.sprint_name));
    Ok(sprints)
}

fn unique_story_overviews(repository: &Repository) -> Vec<StoryOverview> {
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

    selected.into_values().map(story_overview).collect()
}

fn flatten_sprint_stories(sprint: &SprintOverview) -> Vec<StoryOverview> {
    let mut stories = Vec::new();
    let mut seen_statuses = BTreeSet::new();

    for status in SPRINT_STATUS_DISPLAY_ORDER {
        seen_statuses.insert(status);
        if let Some(items) = sprint.stories_by_status.get(status) {
            stories.extend(items.iter().cloned());
        }
    }

    for (status, items) in &sprint.stories_by_status {
        if !seen_statuses.contains(status.as_str()) {
            stories.extend(items.iter().cloned());
        }
    }

    stories
}

fn sprint_overview_from_spec(
    repository: &Repository,
    spec: &SprintFolderSpec,
    today: NaiveDate,
) -> SprintOverview {
    let mut stories_by_status = SPRINT_STATUS_DISPLAY_ORDER
        .iter()
        .map(|status| (status.to_string(), Vec::new()))
        .collect::<BTreeMap<_, _>>();

    let mut blocked_work = Vec::new();

    for story in repository.stories.iter().filter(|story| {
        story.frontmatter.get("sprint").map(String::as_str) == Some(spec.sprint_name.as_str())
    }) {
        let overview = story_overview(story);
        stories_by_status
            .entry(overview.status.clone())
            .or_default()
            .push(overview.clone());

        if overview.status == "blocked" {
            blocked_work.push(BlockedWorkItem {
                story_id: overview.id.clone(),
                story_title: overview.title.clone(),
                task_id: None,
                task_title: None,
            });
        }

        if let Some(task_file) = &story.task_file {
            for task in task_file
                .tasks
                .iter()
                .filter(|task| task.normalized_status == "blocked")
            {
                blocked_work.push(BlockedWorkItem {
                    story_id: overview.id.clone(),
                    story_title: overview.title.clone(),
                    task_id: Some(task.id.clone()),
                    task_title: Some(task.title.clone()),
                });
            }
        }
    }

    for stories in stories_by_status.values_mut() {
        stories.sort_by(|left, right| left.id.cmp(&right.id));
    }

    SprintOverview {
        sprint_name: spec.sprint_name.clone(),
        headline: spec.headline.clone(),
        sprint_goal: spec.sprint_goal.clone(),
        start_date: spec.start_date.format("%Y-%m-%d").to_string(),
        end_date: spec.end_date.format("%Y-%m-%d").to_string(),
        readme_path: relative_path(&repository.repo_root, &spec.readme_path),
        readme_status: spec.readme_status.clone(),
        stories_by_status,
        blocked_work,
        warnings: sprint_warnings(&repository.repo_root, repository, spec, today),
    }
}

fn select_current_sprint(sprints: &[SprintOverview], today: NaiveDate) -> Result<SprintOverview> {
    let current_sprints = sprints
        .iter()
        .filter(|sprint| {
            let start_date = NaiveDate::parse_from_str(&sprint.start_date, "%Y-%m-%d").ok();
            let end_date = NaiveDate::parse_from_str(&sprint.end_date, "%Y-%m-%d").ok();
            match (start_date, end_date) {
                (Some(start_date), Some(end_date)) => date_in_range(today, start_date, end_date),
                _ => false,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    let active_readmes = sprints
        .iter()
        .filter(|sprint| sprint.readme_status.as_deref() == Some("active"))
        .cloned()
        .collect::<Vec<_>>();

    match current_sprints.as_slice() {
        [current] => Ok(current.clone()),
        [] => match active_readmes.as_slice() {
            [current] => Ok(current.clone()),
            [] => Err(anyhow!(
                "No sprint folder date range includes {}.",
                today.format("%Y-%m-%d")
            )),
            _ => Err(anyhow!(
                "No sprint folder date range includes {} and multiple sprint READMEs are marked active: {}. Run `kanban doctor` to inspect the mismatch.",
                today.format("%Y-%m-%d"),
                active_readmes
                    .iter()
                    .map(|sprint| sprint.sprint_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        },
        _ => Err(anyhow!(
            "Multiple sprint folders include {}: {}. Run `kanban doctor` to inspect the overlap.",
            today.format("%Y-%m-%d"),
            current_sprints
                .iter()
                .map(|sprint| sprint.sprint_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn find_story_in_repository(repository: &Repository, story_id: &str) -> Option<StoryDetails> {
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
    let task_file_path = story
        .task_file
        .as_ref()
        .map(|task_file| task_file.relative_path.clone());

    Some(StoryDetails {
        story: story_overview(story),
        task_file_path,
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

fn doctor_findings_for_sprint(
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

fn discover_sprint_folder_specs(config: &KanbanConfig) -> Result<Vec<SprintFolderSpec>> {
    let sprints_root = config.sprints_path();
    let mut specs = Vec::new();

    for entry in fs::read_dir(&sprints_root)
        .with_context(|| format!("read sprint root {}", sprints_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
        else {
            continue;
        };

        let Some((sprint_id, file_headline)) = parse_sprint_file_name(&file_name) else {
            continue;
        };

        let readme_path = path.clone();
        let readme = parse_sprint_readme(&readme_path)?;
        let start_date = readme.start_date.ok_or_else(|| {
            anyhow!(
                "Sprint README is missing start_date: {}",
                readme_path.display()
            )
        })?;
        let end_date = readme.end_date.ok_or_else(|| {
            anyhow!(
                "Sprint README is missing end_date: {}",
                readme_path.display()
            )
        })?;
        let headline = readme.headline.clone().unwrap_or(file_headline);
        if readme.sprint.as_deref() != Some(sprint_id.as_str()) {
            bail!(
                "Sprint README field `sprint` must match folder sprint id {sprint_id}: {}",
                readme_path.display()
            );
        }

        specs.push(SprintFolderSpec {
            sprint_name: file_name.trim_end_matches(".md").to_string(),
            headline,
            sprint_goal: readme.sprint_goal,
            start_date,
            end_date,
            readme_path,
            readme_status: readme.status,
        });
    }

    specs.sort_by(|left, right| left.sprint_name.cmp(&right.sprint_name));
    Ok(specs)
}

fn parse_sprint_readme(readme_path: &Path) -> Result<SprintReadmeInfo> {
    let markdown = fs::read_to_string(readme_path)
        .with_context(|| format!("read sprint summary {}", readme_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    Ok(SprintReadmeInfo {
        sprint: parsed.frontmatter.get("sprint").cloned(),
        headline: parsed.frontmatter.get("headline").cloned(),
        sprint_goal: extract_markdown_section(&parsed.body, "Sprint Goal"),
        status: parsed.frontmatter.get("status").cloned(),
        start_date: parsed
            .frontmatter
            .get("start_date")
            .and_then(|value| parse_markdown_date(value)),
        end_date: parsed
            .frontmatter
            .get("end_date")
            .and_then(|value| parse_markdown_date(value)),
    })
}

fn validate_sprint_readmes(config: &KanbanConfig) -> Result<Vec<ValidationIssue>> {
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

fn readme_table_value(markdown: &str, key: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        if !line.starts_with('|') {
            return None;
        }

        let parts = line
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() != 2 || parts[0] != key {
            return None;
        }

        Some(parts[1].trim_matches('`').to_string())
    })
}

fn parse_sprint_file_name(file_name: &str) -> Option<(String, String)> {
    let pattern = Regex::new(SPRINT_FILE_PATTERN).expect("valid sprint file regex");
    let captures = pattern.captures(file_name)?;
    let sprint_id = captures.get(1)?.as_str().to_string();
    let headline = captures.get(2)?.as_str().to_string();
    Some((sprint_id, headline))
}

fn parse_sprint_number(sprint_name: &str) -> Option<u32> {
    let prefix = sprint_name.strip_prefix('S')?;
    let number = prefix.split_once('.')?.0;
    number.parse().ok()
}

fn current_timestamp_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string()
}

fn current_git_assignee(repo_root: &Path) -> Result<String> {
    let name = git_config_value(repo_root, "user.name")?;
    let email = git_config_value(repo_root, "user.email")?;
    if name.is_empty() || email.is_empty() {
        bail!(
            "Git user.name and user.email must be configured before moving a story to in-progress."
        );
    }
    Ok(format!("{name} <{email}>"))
}

fn validate_assignee_override(assignee: &str) -> Result<String> {
    let trimmed = assignee.trim();
    let pattern =
        Regex::new(r"^[^<>\s].*\s<[^<>\s@]+@[^<>\s@]+>$").expect("valid assignee validation regex");
    if pattern.is_match(trimmed) {
        Ok(trimmed.to_string())
    } else {
        bail!("Assignee must use the format `Name <email>`.");
    }
}

fn git_config_value(repo_root: &Path, key: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("config")
        .arg(key)
        .output()
        .with_context(|| format!("read git config {key}"))?;
    if !output.status.success() {
        bail!("Git config {key} is not set.");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn first_weekday_after(date: NaiveDate, weekday: Weekday) -> NaiveDate {
    let mut current = date + Days::new(1);
    while current.weekday() != weekday {
        current = current + Days::new(1);
    }
    current
}

fn first_weekday_on_or_after(date: NaiveDate, weekday: Weekday) -> NaiveDate {
    let mut current = date;
    while current.weekday() != weekday {
        current = current + Days::new(1);
    }
    current
}

fn slugify_headline(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.trim().chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn render_sprint_file_template(
    sprint_id: &str,
    headline: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> String {
    format!(
        "---\nsprint: {sprint_id}\nheadline: {headline}\nstart_date: {}\nend_date: {}\nstatus: planned\nwip_limit: null\n---\n\n# {sprint_id}: {headline}\n\n## Sprint Goal\n\nTBD\n\n## Notes For Review / Demo\n\n- Sprint created by `kanban sprint create`.\n\n## End-Of-Sprint Summary\n\nSprint not started yet.\n\n## Expected Carry-Over / Unfinished Stories\n\nNot determined yet.\n\n{}\n",
        start_date.format("%Y-%m-%d"),
        end_date.format("%Y-%m-%d"),
        render_sprint_roster(&[]).trim_end()
    )
}

fn render_sprint_roster(rows: &[(String, String)]) -> String {
    let mut out = format!("{ROSTER_HEADING}\n\n");
    for column in ROSTER_COLUMN_ORDER {
        let mut ids: Vec<&str> = rows
            .iter()
            .filter(|(_, status)| status == column)
            .map(|(id, _)| id.as_str())
            .collect();
        ids.sort_unstable();
        if ids.is_empty() {
            out.push_str(&format!("- {column}: —\n"));
        } else {
            out.push_str(&format!("- {column}: {}\n", ids.join(", ")));
        }
    }
    out
}

fn replace_roster_in_body(body: &str, roster: &str) -> String {
    let trimmed = match body.find(ROSTER_HEADING) {
        Some(idx) => body[..idx].trim_end().to_string(),
        None => body.trim_end().to_string(),
    };
    format!("{trimmed}\n\n{roster}")
}

fn frontmatter_region(markdown: &str) -> Result<&str> {
    let normalized = markdown.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") {
        bail!("Markdown file is missing YAML frontmatter.");
    }
    let mut offset = 0;
    for (index, line) in normalized.split_inclusive('\n').enumerate() {
        offset += line.len();
        if index > 0 && line.trim_end() == "---" {
            return Ok(&markdown[..offset]);
        }
    }
    bail!("Markdown file has an unclosed frontmatter block.")
}

fn regenerate_sprint_roster(config: &KanbanConfig, sprint_name: &str) -> Result<bool> {
    let sprint_file = config.sprints_path().join(format!("{sprint_name}.md"));
    if !sprint_file.is_file() {
        return Ok(false);
    }
    let repository = read_repository(&config.repo_root)?;
    let mut rows = repository
        .stories
        .iter()
        .filter(|story| story.frontmatter.get("sprint").map(String::as_str) == Some(sprint_name))
        .filter_map(|story| {
            Some((
                story.frontmatter.get("id")?.clone(),
                story.frontmatter.get("status").cloned().unwrap_or_default(),
            ))
        })
        .collect::<Vec<_>>();
    rows.sort();

    let content = fs::read_to_string(&sprint_file)
        .with_context(|| format!("read sprint file {}", sprint_file.display()))?;
    let parsed = parse_frontmatter(&content);
    let fm_block = frontmatter_region(&content)?;
    let new_body = replace_roster_in_body(&parsed.body, &render_sprint_roster(&rows));
    let updated = format!("{fm_block}{new_body}\n");
    if updated == content {
        return Ok(false);
    }
    fs::write(&sprint_file, updated)
        .with_context(|| format!("write sprint file {}", sprint_file.display()))?;
    Ok(true)
}

pub fn sync_sprint_rosters(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let config = load_kanban_config(repo_root)?;
    let mut changed = Vec::new();
    for name in list_sprint_names(&config.repo_root)? {
        if regenerate_sprint_roster(&config, &name)? {
            changed.push(name);
        }
    }
    Ok(changed)
}

fn normalize_story_status_input(status: &str) -> Result<String> {
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

fn normalize_task_status_for_write(status: &str) -> Result<String> {
    let normalized = normalize_task_status(status);
    if CANONICAL_TASK_STATUSES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        bail!("Unsupported task status: {status}");
    }
}

pub fn update_story_frontmatter_markdown(
    markdown: &str,
    updates: &[(&str, Option<String>)],
) -> Result<String> {
    let normalized = markdown.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    if !normalized.starts_with("---\n") {
        bail!("Story file is missing YAML frontmatter.");
    }
    let closing_index = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index))
        .ok_or_else(|| anyhow!("Story file has an unclosed frontmatter block."))?;

    let mut output = Vec::new();
    output.push("---".to_string());
    for line in &lines[1..closing_index] {
        if line.trim().is_empty() {
            output.push(String::new());
            continue;
        }
        if let Some((key, _)) = line.split_once(':') {
            let key = key.trim();
            if let Some((_, value)) = updates.iter().find(|(candidate, _)| *candidate == key) {
                output.push(format!("{key}: {}", value.clone().unwrap_or_default()));
                continue;
            }
        }
        output.push((*line).to_string());
    }
    output.push("---".to_string());
    output.extend(
        lines[(closing_index + 1)..]
            .iter()
            .map(|line| (*line).to_string()),
    );
    Ok(output.join("\n"))
}

pub fn upsert_frontmatter_markdown(
    markdown: &str,
    updates: &[(&str, Option<String>)],
) -> Result<String> {
    let normalized = markdown.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    if !normalized.starts_with("---\n") {
        bail!("Story file is missing YAML frontmatter.");
    }
    let closing_index = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index))
        .ok_or_else(|| anyhow!("Story file has an unclosed frontmatter block."))?;

    let parsed = parse_frontmatter(&normalized);
    let mut output = Vec::new();
    let mut applied = BTreeSet::new();

    output.push("---".to_string());
    for line in &lines[1..closing_index] {
        if line.trim().is_empty() {
            output.push(String::new());
            continue;
        }
        if let Some((key, _)) = line.split_once(':') {
            let key = key.trim();
            if let Some((_, value)) = updates.iter().find(|(candidate, _)| *candidate == key) {
                applied.insert(key.to_string());
                output.push(format!("{key}: {}", value.clone().unwrap_or_default()));
                continue;
            }
        }
        output.push((*line).to_string());
    }

    for (key, value) in updates {
        if parsed.frontmatter_keys.contains(*key) || applied.contains(*key) {
            continue;
        }
        output.push(format!("{key}: {}", value.clone().unwrap_or_default()));
    }

    output.push("---".to_string());
    output.extend(
        lines[(closing_index + 1)..]
            .iter()
            .map(|line| (*line).to_string()),
    );
    Ok(output.join("\n"))
}

fn upsert_story_frontmatter_file(
    file_path: &Path,
    updates: &[(&str, Option<String>)],
) -> Result<()> {
    let markdown = fs::read_to_string(file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let updated = upsert_frontmatter_markdown(&markdown, updates)?;
    fs::write(file_path, updated)
        .with_context(|| format!("write story file {}", file_path.display()))?;
    Ok(())
}

fn doctor_timestamp_input(input: &DoctorFixInput) -> Result<String> {
    let timestamp = input.value.clone().unwrap_or_else(current_timestamp_string);
    validate_doctor_timestamp(&timestamp)?;
    Ok(timestamp)
}

fn doctor_timestamp_input_with_preview(
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

fn validate_doctor_timestamp(timestamp: &str) -> Result<()> {
    let timestamp_pattern = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{4}$")
        .expect("valid timestamp regex");
    if !timestamp_pattern.is_match(timestamp) {
        bail!("Enter a timestamp as local ISO 8601 with numeric timezone offset.");
    }
    Ok(())
}

fn date_only_timestamp_from_issue_or_file(
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

fn doctor_prompt_for_readme_field(story: Option<&Story>, field_name: &str) -> DoctorPrompt {
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

fn doctor_readme_field_value(
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

fn render_empty_task_file(story_id: &str, sprint_name: &str) -> String {
    format!(
        "# Tasks for {story_id}\n\nParent User Story: {story_id}\nSprint: {sprint_name}\n\n---\n"
    )
}

fn find_sprint_story_for_write<'a>(
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

fn find_story_for_write<'a>(repository: &'a Repository, story_id: &str) -> Result<&'a Story> {
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

fn next_task_id(story: &Story, task_file: &TaskFile) -> String {
    let story_id = story.frontmatter.get("id").cloned().unwrap_or_default();
    let next_number = task_file
        .tasks
        .iter()
        .filter_map(|task| task.id.rsplit('-').next()?.parse::<u32>().ok())
        .max()
        .map(|value| value + 1)
        .unwrap_or(1);
    format!("TASK-{story_id}-{next_number:03}")
}

fn append_task_markdown(
    markdown: &str,
    task_id: &str,
    title: &str,
    status: &str,
    tags: &[String],
    description: &str,
) -> String {
    let mut output = markdown.trim_end().to_string();
    if !output.is_empty() {
        output.push_str("\n\n");
    }
    output.push_str("---\n\n");
    output.push_str(&render_task_block(
        task_id,
        title,
        status,
        tags,
        description,
    ));
    output.push_str("\n\n---\n");
    output
}

fn rewrite_task_markdown(
    markdown: &str,
    task_id: &str,
    status: Option<&str>,
    title: Option<&str>,
    tags: Option<&[String]>,
    description: Option<&str>,
) -> Result<String> {
    let normalized = markdown.replace("\r\n", "\n");
    let heading_pattern = Regex::new(TASK_HEADING_PATTERN).expect("valid task heading regex");
    let matches: Vec<_> = heading_pattern
        .captures_iter(&normalized)
        .filter_map(|captures| {
            let full = captures.get(0)?;
            let id = captures.get(1)?.as_str().to_string();
            let title = captures.get(2)?.as_str().trim().to_string();
            Some((full.start(), full.end(), id, title))
        })
        .collect();

    let normalized_task_id = task_id.trim().to_ascii_uppercase();
    let mut rewritten = String::new();
    let mut cursor = 0;
    let mut found = false;

    for (index, (start, block_start, id, existing_title)) in matches.iter().enumerate() {
        let block_end = matches
            .get(index + 1)
            .map(|next| next.0)
            .unwrap_or(normalized.len());
        rewritten.push_str(&normalized[cursor..*start]);
        let block = &normalized[*block_start..block_end];
        if id.eq_ignore_ascii_case(&normalized_task_id) {
            let existing_status = capture_line_value(block, "Status").unwrap_or_default();
            let existing_tags = capture_line_value(block, "Tags")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            let existing_description = capture_description(block);
            rewritten.push_str(&render_task_block(
                id,
                title.unwrap_or(existing_title),
                status.unwrap_or(existing_status.trim()),
                tags.unwrap_or(existing_tags.as_slice()),
                description.unwrap_or(existing_description.as_str()),
            ));
            found = true;
        } else {
            rewritten.push_str(&normalized[*start..block_end]);
        }
        cursor = block_end;
    }

    rewritten.push_str(&normalized[cursor..]);
    if found {
        Ok(rewritten)
    } else {
        bail!("Task not found: {normalized_task_id}");
    }
}

fn render_task_block(
    task_id: &str,
    title: &str,
    status: &str,
    tags: &[String],
    description: &str,
) -> String {
    format!(
        "## {task_id} - {}\n\nStatus: {}\nTags: {}\n\nDescription:\n{}",
        title.trim(),
        display_task_status(status),
        tags.join(", "),
        description.trim()
    )
}

fn display_task_status(status: &str) -> &'static str {
    match status {
        "todo" => "To Do",
        "in-progress" => "In Progress",
        "blocked" => "Blocked",
        "done" => "Done",
        _ => "To Do",
    }
}

fn update_sprint_summary_for_rollover(
    markdown: &str,
    sprint_name: &str,
    next_sprint_name: &str,
    completed_story_ids: &[String],
    carried_story_ids: &[String],
) -> String {
    let end_summary = if completed_story_ids.is_empty() {
        format!("Sprint closed. No stories were completed in `{sprint_name}` before rollover.")
    } else {
        format!(
            "Sprint closed. Completed stories in `{sprint_name}`: {}.",
            completed_story_ids.join(", ")
        )
    };
    let carry_over = if carried_story_ids.is_empty() {
        "No unfinished stories were moved forward.".to_string()
    } else {
        format!(
            "Moved to `{next_sprint_name}`: {}.",
            carried_story_ids.join(", ")
        )
    };
    let updated = replace_markdown_section(markdown, "End-Of-Sprint Summary", &end_summary);
    replace_markdown_section(
        &updated,
        "Expected Carry-Over / Unfinished Stories",
        &carry_over,
    )
}

fn replace_markdown_section(markdown: &str, heading: &str, new_content: &str) -> String {
    let normalized = markdown.replace("\r\n", "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let target_heading = format!("## {heading}");
    let Some(start) = lines.iter().position(|line| line.trim() == target_heading) else {
        let mut output = normalized.trim_end().to_string();
        output.push_str("\n\n");
        output.push_str(&target_heading);
        output.push_str("\n\n");
        output.push_str(new_content.trim());
        output.push('\n');
        return output;
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());

    let mut output = Vec::new();
    output.extend(lines[..=start].iter().map(|line| (*line).to_string()));
    output.push(String::new());
    output.extend(new_content.trim().lines().map(|line| line.to_string()));
    output.push(String::new());
    output.extend(lines[end..].iter().map(|line| (*line).to_string()));
    output.join("\n")
}

fn parse_markdown_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim_matches('`').trim(), "%Y-%m-%d").ok()
}

fn date_only_timestamp(value: &str) -> Option<String> {
    let date = parse_markdown_date(value)?;
    let midnight = date.and_hms_opt(0, 0, 0)?;
    let local_midnight = Local.from_local_datetime(&midnight).earliest()?;
    Some(local_midnight.format("%Y-%m-%dT%H:%M:%S%z").to_string())
}

fn date_only_timestamp_from_file(file_path: &Path, field_name: &str) -> Result<Option<String>> {
    let markdown = fs::read_to_string(file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    Ok(parsed
        .frontmatter
        .get(field_name)
        .and_then(|value| date_only_timestamp(value)))
}

fn date_in_range(today: NaiveDate, start_date: NaiveDate, end_date: NaiveDate) -> bool {
    today >= start_date && today <= end_date
}

fn sprint_warnings(
    repo_root: &Path,
    repository: &Repository,
    spec: &SprintFolderSpec,
    today: NaiveDate,
) -> Vec<String> {
    doctor_findings_for_sprint(repo_root, repository, spec, today)
        .into_iter()
        .map(|finding| finding.message)
        .collect()
}

fn story_overview(story: &Story) -> StoryOverview {
    StoryOverview {
        id: story.frontmatter.get("id").cloned().unwrap_or_else(|| {
            story
                .file_name
                .trim_end_matches(STORY_FILE_SUFFIX)
                .to_string()
        }),
        title: story_title(&story.body).unwrap_or_else(|| story.file_name.clone()),
        status: story.frontmatter.get("status").cloned().unwrap_or_default(),
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

fn story_title(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let title = line.strip_prefix("# ")?.trim();
        let title = title
            .strip_prefix("User Story: ")
            .or_else(|| title.strip_prefix("Epic: "))
            .unwrap_or(title);
        Some(title.to_string())
    })
}

fn extract_markdown_section(body: &str, heading: &str) -> Option<String> {
    let normalized = body.replace("\r\n", "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let target_heading = format!("## {heading}");
    let start = lines
        .iter()
        .position(|line| line.trim() == target_heading)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());

    let section_lines = lines[(start + 1)..end]
        .iter()
        .copied()
        .skip_while(|line| line.trim().is_empty() || line.trim() == "---")
        .collect::<Vec<_>>();
    let mut section = section_lines.join("\n").trim().to_string();
    while section.ends_with("\n---") || section == "---" {
        section = section.trim_end_matches("---").trim_end().to_string();
    }
    (!section.is_empty()).then_some(section)
}

fn normalize_phase_input(phase: &str) -> Result<String> {
    let digits = phase
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(anyhow!(
            "Phase must contain a numeric identifier, for example `1` or `F1`."
        ));
    }

    let trimmed = digits.trim_start_matches('0');
    Ok(if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    })
}

fn parse_scalar(raw_value: &str) -> String {
    let value = raw_value.trim();
    if value.is_empty() {
        return String::new();
    }
    if value == "~" {
        return "~".to_string();
    }
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return value[1..value.len() - 1].to_string();
    }
    value.to_string()
}

fn normalize_task_status(status: &str) -> String {
    match status.trim().to_ascii_lowercase().as_str() {
        "to do" => "todo".to_string(),
        "in progress" => "in-progress".to_string(),
        other => other.to_string(),
    }
}

fn capture_line_value(block: &str, prefix: &str) -> Option<String> {
    block.lines().find_map(|line| {
        let (left, right) = line.split_once(':')?;
        (left.trim() == prefix).then(|| right.trim().to_string())
    })
}

fn capture_description(block: &str) -> String {
    let marker = "Description:\n";
    let Some(start) = block.find(marker) else {
        return String::new();
    };
    let mut description = block[(start + marker.len())..].trim().to_string();
    if let Some(stripped) = description.strip_suffix("---") {
        description = stripped.trim_end().to_string();
    }
    description
}

fn validate_timestamp_field(
    story: &Story,
    issues: &mut Vec<ValidationIssue>,
    field_name: &str,
    allow_empty: bool,
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
    if !timestamp_pattern.is_match(value) {
        add_issue(
            story,
            issues,
            format!("invalid-timestamp:{field_name}"),
            format!(
                "Frontmatter field \"{field_name}\" must use local ISO 8601 with numeric timezone offset."
            ),
        );
    }
}

fn assignee_required(story: &Story) -> bool {
    matches!(
        story.frontmatter.get("status").map(String::as_str),
        Some("in-progress" | "ready-for-qa" | "blocked" | "done")
    ) || story
        .frontmatter
        .get("work_started")
        .map(String::as_str)
        .is_some_and(|value| !value.is_empty())
}

fn add_issue(
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

fn relative_path(repo_root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(repo_root).unwrap_or(path).to_path_buf()
}

fn to_forward_slashes(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

struct StoryLocation {
    sprint_name: Option<String>,
}

fn story_location(file_path: &Path, config: &KanbanConfig) -> StoryLocation {
    let _ = (file_path, config);
    StoryLocation { sprint_name: None }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn validate_story_reports_invalid_timestamps_on_draft_backlog_fixture() {
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
        assert!(rules.contains(&"invalid-timestamp:created"));
        assert!(rules.contains(&"invalid-timestamp:updated"));
    }

    #[test]
    fn doctor_fix_normalizes_date_only_timestamp_as_info() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = temp_root
            .path()
            .join("delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-050-test-draft-story.md");

        fs::create_dir_all(story_path.parent().unwrap()).unwrap();
        fs::write(
            &story_path,
            "---\nid: US-F1-050\ntype: user-story\nstatus: draft\nepic: EP-F1-06\nsprint: ~\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-31\n---\n# User Story\n",
        )
        .unwrap();

        let issue = collect_doctor_issues_for_story(temp_root.path(), "US-F1-050")
            .unwrap()
            .into_iter()
            .find(|issue| issue.rule == "invalid-timestamp:updated")
            .unwrap();

        assert_eq!(issue.severity, "info");
        assert_eq!(issue.fix_kind, DoctorFixKind::Automatic);
        assert_eq!(issue.prompt, DoctorPrompt::None);
        assert_eq!(
            issue.fix_preview,
            Some(DoctorFixPreview {
                field_name: "updated".to_string(),
                old_value: "2026-05-31".to_string(),
                new_value: date_only_timestamp("2026-05-31").unwrap(),
            })
        );

        let result =
            apply_doctor_fix(temp_root.path(), &issue, &DoctorFixInput::default()).unwrap();
        let expected_timestamp = date_only_timestamp("2026-05-31").unwrap();
        let updated = fs::read_to_string(&story_path).unwrap();

        assert!(result.message.contains("INFO"));
        assert!(updated.contains(&format!("updated: {expected_timestamp}")));
        assert!(
            collect_doctor_issues_for_story(temp_root.path(), "US-F1-050")
                .unwrap()
                .into_iter()
                .all(|issue| issue.rule != "invalid-timestamp:updated")
        );
    }

    #[test]
    fn validate_story_requires_assignee_after_work_has_started() {
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
