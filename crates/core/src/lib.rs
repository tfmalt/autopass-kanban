use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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
            .map(|source_path| file_path.parent().unwrap().join(source_path))
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
    let folder_status = status_folder.as_deref().and_then(|folder| match folder {
        "01.todo" => Some("todo".to_string()),
        "02.in-progress" => Some("in-progress".to_string()),
        "03.ready-for-qa" => Some("ready-for-qa".to_string()),
        "04.done" => Some("done".to_string()),
        "99.blocked" => Some("blocked".to_string()),
        _ => None,
    });

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
}
