use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{Local, NaiveDate};
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

const REQUIRED_STORY_FIELDS: [&str; 11] = [
    "id",
    "type",
    "status",
    "epic",
    "sprint",
    "assignee",
    "story_points",
    "work_started",
    "work_done",
    "created",
    "updated",
];

const REQUIRED_SPRINT_FIELDS: [&str; 3] = ["source_path", "task_file", "activated"];
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
const ALLOWED_STORY_POINTS: [&str; 10] = ["2", "3", "5", "8", "13", "XS", "S", "M", "L", "XL"];
const TASK_HEADING_PATTERN: &str = r"(?m)^##\s+(TASK-[A-Z0-9-]+)\s+-\s+(.+)$";
const STORY_FILE_PREFIX: &str = "US-";
const STORY_FILE_SUFFIX: &str = ".md";
const TASK_FILE_SUFFIX: &str = ".tasks.md";
const SPRINT_FOLDER_PATTERN: &str = r"^S\d+\.(\d{4}-\d{2}-\d{2})--(\d{4}-\d{2}-\d{2})\.(.+)$";
const SPRINT_STATUS_FOLDERS: [(&str, &str); 5] = [
    ("01.todo", "todo"),
    ("02.in-progress", "in-progress"),
    ("03.ready-for-qa", "ready-for-qa"),
    ("04.done", "done"),
    ("99.blocked", "blocked"),
];
const SPRINT_STATUS_DISPLAY_ORDER: [&str; 5] = ["todo", "in-progress", "ready-for-qa", "done", "blocked"];

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
pub enum StoryKind {
    Backlog,
    Sprint,
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
    pub kind: StoryKind,
    pub sprint_name: Option<String>,
    pub status_folder: Option<String>,
    pub folder_status: Option<String>,
    pub task_file: Option<TaskFile>,
    pub source_story_path: Option<PathBuf>,
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
    pub kind: StoryKind,
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
    pub start_date: String,
    pub end_date: String,
    pub readme_path: PathBuf,
    pub readme_status: Option<String>,
    pub stories_by_status: BTreeMap<String, Vec<StoryOverview>>,
    pub blocked_work: Vec<BlockedWorkItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseOverview {
    pub phase: String,
    pub stories: Vec<StoryOverview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryDetails {
    pub story: StoryOverview,
    pub source_story_path: Option<PathBuf>,
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
struct SprintFolderSpec {
    sprint_name: String,
    headline: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    readme_path: PathBuf,
    readme_status: Option<String>,
    readme_start_date: Option<NaiveDate>,
    readme_end_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SprintReadmeInfo {
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
    let backlog_root = repo_root.as_ref().join("doc/backlog");
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
    let file_path = fs::canonicalize(file_path.as_ref())
        .with_context(|| format!("resolve story file {}", file_path.as_ref().display()))?;
    let markdown = fs::read_to_string(&file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    let location = story_location(&file_path);
    let source_story_path = if matches!(location.kind, StoryKind::Sprint) {
        parsed
            .frontmatter
            .get("source_path")
            .filter(|value| !value.is_empty())
            .map(|source_path| normalize_path(file_path.parent().unwrap().join(source_path)))
    } else {
        None
    };

    let task_file = if matches!(location.kind, StoryKind::Sprint) {
        parsed
            .frontmatter
            .get("task_file")
            .filter(|value| !value.is_empty())
            .map(|task_file_name| {
                let task_file_path = file_path.parent().unwrap().join(task_file_name);
                if task_file_path.exists() {
                    read_task_file(&task_file_path, repo_root)
                } else {
                    Ok(TaskFile {
                        exists: false,
                        relative_path: relative_path(repo_root, &task_file_path),
                        file_path: task_file_path,
                        tasks: Vec::new(),
                        summary: TaskSummary::default(),
                        markdown: None,
                    })
                }
            })
            .transpose()?
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
        kind: location.kind,
        sprint_name: location.sprint_name,
        status_folder: location.status_folder,
        folder_status: location.folder_status,
        task_file,
        source_story_path,
    })
}

pub fn read_repository(repo_root: impl AsRef<Path>) -> Result<Repository> {
    let repo_root = fs::canonicalize(repo_root.as_ref())
        .with_context(|| format!("resolve repo root {}", repo_root.as_ref().display()))?;
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
    let phase_marker = format!("/doc/backlog/phase-{phase_number}-");
    let mut stories = repository
        .stories
        .iter()
        .filter(|story| matches!(story.kind, StoryKind::Backlog))
        .filter(|story| to_forward_slashes(&story.file_path).contains(&phase_marker))
        .map(story_overview)
        .collect::<Vec<_>>();

    stories.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(PhaseOverview {
        phase: format!("F{phase_number}"),
        stories,
    })
}

pub fn find_story(repo_root: impl AsRef<Path>, story_id: &str) -> Result<Option<StoryDetails>> {
    let repository = read_repository(repo_root)?;
    Ok(find_story_in_repository(&repository, story_id))
}

pub fn doctor_repository(repo_root: impl AsRef<Path>) -> Result<Vec<DoctorFinding>> {
    doctor_repository_at_date(repo_root, Local::now().date_naive())
}

pub fn doctor_repository_at_date(
    repo_root: impl AsRef<Path>,
    today: NaiveDate,
) -> Result<Vec<DoctorFinding>> {
    let repository = read_repository(repo_root)?;
    let validation = validate_repository(&repository.repo_root)?;
    let sprint_specs = discover_sprint_folder_specs(&repository.repo_root)?;
    let mut findings = Vec::new();

    for issue in validation.issues {
        findings.push(DoctorFinding {
            severity: "error".to_string(),
            scope: issue.file_path.display().to_string(),
            message: format!("[{}] {}", issue.rule, issue.message),
        });
    }

    let current_by_date: Vec<_> = sprint_specs
        .iter()
        .filter(|spec| date_in_range(today, spec.start_date, spec.end_date))
        .collect();

    if current_by_date.is_empty() {
        findings.push(DoctorFinding {
            severity: "warning".to_string(),
            scope: "sprints".to_string(),
            message: format!(
                "No sprint folder date range includes {}. Current sprint detection cannot succeed until sprint dates are corrected.",
                today.format("%Y-%m-%d")
            ),
        });
    }

    if current_by_date.len() > 1 {
        findings.push(DoctorFinding {
            severity: "error".to_string(),
            scope: "sprints".to_string(),
            message: format!(
                "Multiple sprint folders include {}: {}.",
                today.format("%Y-%m-%d"),
                current_by_date
                    .iter()
                    .map(|spec| spec.sprint_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    for spec in sprint_specs {
        findings.extend(doctor_findings_for_sprint(&spec, today));
    }

    Ok(findings)
}

pub fn validate_story(story: &Story) -> Vec<ValidationIssue> {
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

    if matches!(story.kind, StoryKind::Sprint) {
        for field_name in REQUIRED_SPRINT_FIELDS {
            if !story.frontmatter_keys.contains(field_name) {
                add_issue(
                    story,
                    &mut issues,
                    format!("missing-field:{field_name}"),
                    format!("Missing required sprint frontmatter field \"{field_name}\"."),
                );
            }
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
        if !ALLOWED_STORY_POINTS.contains(&story_points) {
            add_issue(
                story,
                &mut issues,
                "invalid-story-points",
                "story_points must be one of 2, 3, 5, 8, 13 or the draft aliases XS, S, M, L, XL."
                    .to_string(),
            );
        }
    }

    validate_timestamp_field(story, &mut issues, "created", false);
    validate_timestamp_field(story, &mut issues, "updated", false);
    validate_timestamp_field(story, &mut issues, "work_started", true);
    validate_timestamp_field(story, &mut issues, "work_done", true);

    if matches!(story.kind, StoryKind::Sprint) {
        validate_timestamp_field(story, &mut issues, "activated", false);

        if let (Some(folder_status), Some(status)) = (
            story.folder_status.as_deref(),
            story.frontmatter.get("status"),
        ) {
            if status != folder_status {
                add_issue(
                    story,
                    &mut issues,
                    "status-folder-mismatch",
                    format!(
                        "Story status \"{status}\" does not match sprint folder status \"{folder_status}\"."
                    ),
                );
            }
        }

        if let (Some(sprint_name), Some(sprint)) = (
            story.sprint_name.as_deref(),
            story.frontmatter.get("sprint"),
        ) {
            if sprint != sprint_name {
                add_issue(
                    story,
                    &mut issues,
                    "sprint-name-mismatch",
                    format!(
                        "Story sprint field \"{sprint}\" does not match parent sprint folder \"{sprint_name}\"."
                    ),
                );
            }
        }

        if let Some(source_story_path) = &story.source_story_path {
            if !to_forward_slashes(source_story_path).contains("/doc/backlog/") {
                add_issue(
                    story,
                    &mut issues,
                    "invalid-source-path",
                    "source_path must resolve to a backlog file inside doc/backlog/.".to_string(),
                );
            }
        } else {
            add_issue(
                story,
                &mut issues,
                "missing-source-path",
                "Sprint stories must point back to their source backlog story.".to_string(),
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

    for story in &repository.stories {
        issues.extend(validate_story(story));
        if !matches!(story.kind, StoryKind::Sprint) {
            continue;
        }

        if let Some(source_story_path) = &story.source_story_path {
            if !source_story_path.exists() {
                add_issue(
                    story,
                    &mut issues,
                    "missing-source-story",
                    format!(
                        "source_path points to a story that does not exist: {}",
                        relative_path(&repository.repo_root, source_story_path).display()
                    ),
                );
            }
        }

        if let Some(task_file) = &story.task_file {
            if !task_file.exists
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
    }

    Ok(ValidationReport {
        repo_root: repository.repo_root,
        stories: repository.stories,
        issues,
    })
}

fn summarize_sprints_from_repository(repository: &Repository) -> Result<Vec<SprintOverview>> {
    let today = Local::now().date_naive();
    let specs = discover_sprint_folder_specs(&repository.repo_root)?;
    let mut sprints = specs
        .iter()
        .map(|spec| sprint_overview_from_spec(repository, spec, today))
        .collect::<Vec<_>>();
    sprints.sort_by(|left, right| left.sprint_name.cmp(&right.sprint_name));
    Ok(sprints)
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

    for story in repository
        .stories
        .iter()
        .filter(|story| matches!(story.kind, StoryKind::Sprint))
        .filter(|story| story.sprint_name.as_deref() == Some(spec.sprint_name.as_str()))
    {
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
        start_date: spec.start_date.format("%Y-%m-%d").to_string(),
        end_date: spec.end_date.format("%Y-%m-%d").to_string(),
        readme_path: relative_path(&repository.repo_root, &spec.readme_path),
        readme_status: spec.readme_status.clone(),
        stories_by_status,
        blocked_work,
        warnings: sprint_warnings(spec, today),
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

    match current_sprints.as_slice() {
        [current] => Ok(current.clone()),
        [] => {
            let active_readmes = sprints
                .iter()
                .filter(|sprint| sprint.readme_status.as_deref() == Some("Active"))
                .map(|sprint| sprint.sprint_name.as_str())
                .collect::<Vec<_>>();

            if active_readmes.is_empty() {
                Err(anyhow!(
                    "No sprint folder date range includes {}.",
                    today.format("%Y-%m-%d")
                ))
            } else {
                Err(anyhow!(
                    "No sprint folder date range includes {} even though README status is Active for {}. Run `kanban doctor` to inspect the mismatch.",
                    today.format("%Y-%m-%d"),
                    active_readmes.join(", ")
                ))
            }
        }
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

    matches.sort_by(|left, right| match (&left.kind, &right.kind) {
        (StoryKind::Sprint, StoryKind::Backlog) => std::cmp::Ordering::Less,
        (StoryKind::Backlog, StoryKind::Sprint) => std::cmp::Ordering::Greater,
        _ => left.relative_path.cmp(&right.relative_path),
    });

    let story = matches.into_iter().next()?;
    let task_file_path = story
        .task_file
        .as_ref()
        .map(|task_file| task_file.relative_path.clone());

    Some(StoryDetails {
        story: story_overview(story),
        source_story_path: story
            .source_story_path
            .as_ref()
            .map(|path| relative_path(&repository.repo_root, path)),
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

fn doctor_findings_for_sprint(spec: &SprintFolderSpec, today: NaiveDate) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();
    let in_current_range = date_in_range(today, spec.start_date, spec.end_date);

    if let Some(readme_start_date) = spec.readme_start_date {
        if readme_start_date != spec.start_date {
            findings.push(DoctorFinding {
                severity: "warning".to_string(),
                scope: spec.sprint_name.clone(),
                message: format!(
                    "README start date {} does not match sprint folder start date {}.",
                    readme_start_date.format("%Y-%m-%d"),
                    spec.start_date.format("%Y-%m-%d")
                ),
            });
        }
    }

    if let Some(readme_end_date) = spec.readme_end_date {
        if readme_end_date != spec.end_date {
            findings.push(DoctorFinding {
                severity: "warning".to_string(),
                scope: spec.sprint_name.clone(),
                message: format!(
                    "README end date {} does not match sprint folder end date {}.",
                    readme_end_date.format("%Y-%m-%d"),
                    spec.end_date.format("%Y-%m-%d")
                ),
            });
        }
    }

    match (in_current_range, spec.readme_status.as_deref()) {
        (true, Some("Active")) => {}
        (true, other) => findings.push(DoctorFinding {
            severity: "warning".to_string(),
            scope: spec.sprint_name.clone(),
            message: format!(
                "Sprint folder dates include {} but README status is {}. Folder dates are authoritative. Run `kanban doctor` after updating the sprint README.",
                today.format("%Y-%m-%d"),
                other.unwrap_or("missing")
            ),
        }),
        (false, Some("Active")) => findings.push(DoctorFinding {
            severity: "warning".to_string(),
            scope: spec.sprint_name.clone(),
            message: format!(
                "README status is Active but {} is outside the sprint folder date range {}..{}. Folder dates are authoritative. Run `kanban doctor` after updating the sprint README.",
                today.format("%Y-%m-%d"),
                spec.start_date.format("%Y-%m-%d"),
                spec.end_date.format("%Y-%m-%d")
            ),
        }),
        _ => {}
    }

    findings
}

fn discover_sprint_folder_specs(repo_root: &Path) -> Result<Vec<SprintFolderSpec>> {
    let sprints_root = repo_root.join("doc/backlog/sprints");
    let mut specs = Vec::new();

    for entry in fs::read_dir(&sprints_root)
        .with_context(|| format!("read sprint root {}", sprints_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(folder_name) = path.file_name().map(|value| value.to_string_lossy().into_owned()) else {
            continue;
        };

        let Some((start_date, end_date, headline)) = parse_sprint_folder_name(&folder_name) else {
            continue;
        };

        let readme_path = path.join("README.md");
        let readme = if readme_path.exists() {
            parse_sprint_readme(&readme_path)?
        } else {
            SprintReadmeInfo {
                status: None,
                start_date: None,
                end_date: None,
            }
        };

        specs.push(SprintFolderSpec {
            sprint_name: folder_name,
            headline,
            start_date,
            end_date,
            readme_path,
            readme_status: readme.status,
            readme_start_date: readme.start_date,
            readme_end_date: readme.end_date,
        });
    }

    specs.sort_by(|left, right| left.sprint_name.cmp(&right.sprint_name));
    Ok(specs)
}

fn parse_sprint_readme(readme_path: &Path) -> Result<SprintReadmeInfo> {
    let markdown = fs::read_to_string(readme_path)
        .with_context(|| format!("read sprint summary {}", readme_path.display()))?;
    Ok(SprintReadmeInfo {
        status: readme_table_value(&markdown, "Sprint Status"),
        start_date: readme_table_value(&markdown, "Start Date")
            .and_then(|value| parse_markdown_date(&value)),
        end_date: readme_table_value(&markdown, "End Date")
            .and_then(|value| parse_markdown_date(&value)),
    })
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

fn parse_sprint_folder_name(folder_name: &str) -> Option<(NaiveDate, NaiveDate, String)> {
    let pattern = Regex::new(SPRINT_FOLDER_PATTERN).expect("valid sprint folder regex");
    let captures = pattern.captures(folder_name)?;
    let start_date = NaiveDate::parse_from_str(captures.get(1)?.as_str(), "%Y-%m-%d").ok()?;
    let end_date = NaiveDate::parse_from_str(captures.get(2)?.as_str(), "%Y-%m-%d").ok()?;
    let headline = captures.get(3)?.as_str().to_string();
    Some((start_date, end_date, headline))
}

fn parse_markdown_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim_matches('`').trim(), "%Y-%m-%d").ok()
}

fn date_in_range(today: NaiveDate, start_date: NaiveDate, end_date: NaiveDate) -> bool {
    today >= start_date && today <= end_date
}

fn sprint_warnings(spec: &SprintFolderSpec, today: NaiveDate) -> Vec<String> {
    doctor_findings_for_sprint(spec, today)
        .into_iter()
        .map(|finding| finding.message)
        .collect()
}

fn story_overview(story: &Story) -> StoryOverview {
    StoryOverview {
        id: story
            .frontmatter
            .get("id")
            .cloned()
            .unwrap_or_else(|| story.file_name.trim_end_matches(STORY_FILE_SUFFIX).to_string()),
        title: story_title(&story.body).unwrap_or_else(|| story.file_name.clone()),
        status: story.frontmatter.get("status").cloned().unwrap_or_default(),
        assignee: story.frontmatter.get("assignee").cloned().unwrap_or_default(),
        story_points: story
            .frontmatter
            .get("story_points")
            .cloned()
            .unwrap_or_default(),
        sprint: story.frontmatter.get("sprint").cloned(),
        kind: story.kind.clone(),
        relative_path: story.relative_path.clone(),
        task_summary: story.task_file.as_ref().map(|task_file| task_file.summary.clone()),
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
    let start = lines.iter().position(|line| line.trim() == target_heading)?;
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
        section = section
            .trim_end_matches("---")
            .trim_end()
            .to_string();
    }
    (!section.is_empty()).then_some(section)
}

fn normalize_phase_input(phase: &str) -> Result<String> {
    let digits = phase.chars().filter(|ch| ch.is_ascii_digit()).collect::<String>();
    if digits.is_empty() {
        return Err(anyhow!("Phase must contain a numeric identifier, for example `1` or `F1`."));
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

fn normalize_path(path: PathBuf) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(&path) {
        return canonical;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

struct StoryLocation {
    kind: StoryKind,
    sprint_name: Option<String>,
    status_folder: Option<String>,
    folder_status: Option<String>,
}

fn story_location(file_path: &Path) -> StoryLocation {
    let path_text = to_forward_slashes(file_path);
    let marker = "/doc/backlog/sprints/";
    let Some(index) = path_text.find(marker) else {
        return StoryLocation {
            kind: StoryKind::Backlog,
            sprint_name: None,
            status_folder: None,
            folder_status: None,
        };
    };

    let remainder = &path_text[(index + marker.len())..];
    let mut parts = remainder.split('/');
    let sprint_name = parts.next().map(ToOwned::to_owned);
    let status_folder = parts.next().map(ToOwned::to_owned);
    let folder_status = status_folder
        .as_deref()
        .and_then(|folder| SPRINT_STATUS_FOLDERS.iter().find_map(|(name, status)| {
            (*name == folder).then(|| (*status).to_string())
        }));

    StoryLocation {
        kind: StoryKind::Sprint,
        sprint_name,
        status_folder,
        folder_status,
    }
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
    fn read_story_file_parses_sprint_story_and_sibling_task_file() {
        let repo_root = repo_root();
        let sprint_story_path = repo_root.join("doc/backlog/sprints/S000.2026-05-18--2026-05-29.getting-started/02.in-progress/US-F1-010-ci-pipeline-build-and-unit-tests.md");

        let story = read_story_file(sprint_story_path, &repo_root).unwrap();

        assert_eq!(story.kind, StoryKind::Sprint);
        assert_eq!(
            story.sprint_name.as_deref(),
            Some("S000.2026-05-18--2026-05-29.getting-started")
        );
        assert_eq!(story.status_folder.as_deref(), Some("02.in-progress"));
        assert_eq!(story.folder_status.as_deref(), Some("in-progress"));
        assert_eq!(
            story.frontmatter.get("id").map(String::as_str),
            Some("US-F1-010")
        );
        assert_eq!(
            story.frontmatter.get("status").map(String::as_str),
            Some("in-progress")
        );

        let task_file = story.task_file.as_ref().unwrap();
        assert!(task_file.exists);
        assert_eq!(task_file.tasks.len(), 4);
        assert_eq!(
            task_file.summary,
            TaskSummary {
                todo: 3,
                in_progress: 1,
                blocked: 0,
                done: 0
            }
        );
    }

    #[test]
    fn validate_story_accepts_representative_in_progress_story_fixture() {
        let repo_root = repo_root();
        let sprint_story_path = repo_root.join("doc/backlog/sprints/S000.2026-05-18--2026-05-29.getting-started/02.in-progress/US-F1-010-ci-pipeline-build-and-unit-tests.md");
        let story = read_story_file(sprint_story_path, &repo_root).unwrap();

        assert!(validate_story(&story).is_empty());
    }

    #[test]
    fn validate_story_reports_missing_fields_and_invalid_timestamps_on_draft_backlog_fixture() {
        let repo_root = repo_root();
        let draft_story_path = repo_root.join("doc/backlog/phase-1-scaffolding/07.verification-of-technology-stack-feasability/US-F1-060-kogito-poc-for-dmn-based-rule-evaluation.md");
        let story = read_story_file(draft_story_path, &repo_root).unwrap();
        let issues = validate_story(&story);
        let rules: Vec<&str> = issues.iter().map(|issue| issue.rule.as_str()).collect();

        assert!(rules.contains(&"missing-field:assignee"));
        assert!(rules.contains(&"invalid-timestamp:created"));
        assert!(rules.contains(&"invalid-timestamp:updated"));
    }

    #[test]
    fn validate_repository_catches_status_mismatch_and_only_requires_task_file_after_work_starts() {
        let temp_root = tempdir().unwrap();
        let sprint_directory = temp_root
            .path()
            .join("doc/backlog/sprints/S123.2026-06-01--2026-06-12.demo/02.in-progress");
        let source_directory = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling");

        fs::create_dir_all(&sprint_directory).unwrap();
        fs::create_dir_all(&source_directory).unwrap();
        fs::write(
            source_directory.join("US-F1-051-build-shared-backlog-parsing-and-validation-core.md"),
            "---\nid: US-F1-051\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S123.2026-06-01--2026-06-12.demo\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n",
        ).unwrap();
        fs::write(
            sprint_directory.join("US-F1-051-build-shared-backlog-parsing-and-validation-core.md"),
            "---\nid: US-F1-051\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S123.2026-06-01--2026-06-12.demo\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\nsource_path: ../../../phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-051-build-shared-backlog-parsing-and-validation-core.md\ntask_file: US-F1-051-build-shared-backlog-parsing-and-validation-core.tasks.md\nactivated: 2026-05-28T14:05:54+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n",
        ).unwrap();

        let validation = validate_repository(temp_root.path()).unwrap();
        let rules: Vec<&str> = validation
            .issues
            .iter()
            .map(|issue| issue.rule.as_str())
            .collect();

        assert!(rules.contains(&"status-folder-mismatch"));
        assert!(!rules.contains(&"missing-task-file"));
    }

    #[test]
    fn summarize_current_sprint_uses_folder_dates_and_warns_when_readme_is_not_active() {
        let temp_root = tempdir().unwrap();
        let sprint_root = temp_root
            .path()
            .join("doc/backlog/sprints/S001.2026-05-18--2026-05-29.foundation");
        let sprint_todo = sprint_root.join("01.todo");
        let backlog_dir = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling");

        fs::create_dir_all(&sprint_todo).unwrap();
        fs::create_dir_all(&backlog_dir).unwrap();
        fs::write(
            sprint_root.join("README.md"),
            "# S001.2026-05-18--2026-05-29.foundation\n\n## Sprint Summary\n\n| Field | Value |\n|-------|-------|\n| Sprint Name | `S001.2026-05-18--2026-05-29.foundation` |\n| Start Date | `2026-05-18` |\n| End Date | `2026-05-29` |\n| Sprint Status | Planned |\n",
        ).unwrap();
        fs::write(
            backlog_dir.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md"),
            "---\nid: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.2026-05-18--2026-05-29.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story: Add read-only CLI for sprint and backlog inspection\n",
        ).unwrap();
        fs::write(
            sprint_todo.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md"),
            "---\nid: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.2026-05-18--2026-05-29.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\nsource_path: ../../../phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md\ntask_file: US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.tasks.md\nactivated: 2026-05-28T14:05:54+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story: Add read-only CLI for sprint and backlog inspection\n",
        ).unwrap();

        let sprint = summarize_current_sprint_at_date(temp_root.path(), NaiveDate::from_ymd_opt(2026, 5, 28).unwrap()).unwrap();

        assert_eq!(sprint.sprint_name, "S001.2026-05-18--2026-05-29.foundation");
        assert!(sprint
            .warnings
            .iter()
            .any(|warning| warning.contains("Folder dates are authoritative")));
    }

    #[test]
    fn summarize_phase_lists_backlog_stories_with_sprint_assignment() {
        let repo_root = repo_root();
        let phase = summarize_phase(&repo_root, "F1").unwrap();

        assert_eq!(phase.phase, "F1");
        assert!(phase.stories.iter().any(|story| {
            story.id == "US-F1-052"
                && story.sprint.as_deref()
                    == Some("S000.2026-05-18--2026-05-29.getting-started")
        }));
    }

    #[test]
    fn find_story_prefers_sprint_copy_and_exposes_acceptance_criteria_and_tasks() {
        let repo_root = repo_root();
        let story = find_story(&repo_root, "US-F1-010").unwrap().unwrap();

        assert_eq!(story.story.kind, StoryKind::Sprint);
        assert!(story
            .acceptance_criteria
            .as_deref()
            .unwrap_or_default()
            .contains("Scenario 1"));
        assert_eq!(story.tasks.len(), 4);
    }

    #[test]
    fn doctor_reports_readme_status_disagreement_with_folder_dates() {
        let temp_root = tempdir().unwrap();
        let sprint_root = temp_root
            .path()
            .join("doc/backlog/sprints/S001.2026-05-18--2026-05-29.foundation");
        let sprint_todo = sprint_root.join("01.todo");
        let backlog_dir = temp_root
            .path()
            .join("doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling");

        fs::create_dir_all(&sprint_todo).unwrap();
        fs::create_dir_all(&backlog_dir).unwrap();
        fs::write(
            sprint_root.join("README.md"),
            "# Sprint\n\n## Sprint Summary\n\n| Field | Value |\n|-------|-------|\n| Sprint Name | `S001.2026-05-18--2026-05-29.foundation` |\n| Start Date | `2026-05-18` |\n| End Date | `2026-05-29` |\n| Sprint Status | Planned |\n",
        ).unwrap();
        fs::write(
            backlog_dir.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md"),
            "---\nid: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.2026-05-18--2026-05-29.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story: Add read-only CLI for sprint and backlog inspection\n",
        ).unwrap();
        fs::write(
            sprint_todo.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md"),
            "---\nid: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.2026-05-18--2026-05-29.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\nsource_path: ../../../phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md\ntask_file: US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.tasks.md\nactivated: 2026-05-28T14:05:54+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story: Add read-only CLI for sprint and backlog inspection\n",
        ).unwrap();

        let findings = doctor_repository_at_date(temp_root.path(), NaiveDate::from_ymd_opt(2026, 5, 28).unwrap()).unwrap();

        assert!(findings.iter().any(|finding| {
            finding.scope == "S001.2026-05-18--2026-05-29.foundation"
                && finding.message.contains("Folder dates are authoritative")
        }));
    }
}
