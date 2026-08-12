#[allow(unused_imports)]
use crate::prelude::*;
use crate::{StoryStatus, parse_assignee_list};

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
    #[serde(rename = "in-progress")]
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
}

/// Shared tally of task statuses, keyed by the exact normalized status
/// string. This is the single place where "which status string maps to
/// which bucket" is decided; callers that need a different fold (e.g.
/// treating unrecognized statuses as "todo", or dropping a bucket
/// entirely) apply that fold when reading the counts back out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskStatusCounts {
    pub todo: usize,
    pub in_progress: usize,
    pub ready_for_qa: usize,
    pub done: usize,
    pub blocked: usize,
    /// Statuses that did not match any known bucket above.
    pub other: usize,
    pub total: usize,
}

impl TaskStatusCounts {
    pub fn count<'a, I>(statuses: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut counts = Self::default();
        for status in statuses {
            counts.total += 1;
            match status {
                "todo" => counts.todo += 1,
                "in-progress" => counts.in_progress += 1,
                "ready-for-qa" => counts.ready_for_qa += 1,
                "done" => counts.done += 1,
                "blocked" => counts.blocked += 1,
                _ => counts.other += 1,
            }
        }
        counts
    }
}

#[cfg(test)]
mod task_status_counts_tests {
    use super::TaskStatusCounts;

    #[test]
    fn empty_iterator_yields_all_zero_counts() {
        let counts = TaskStatusCounts::count(std::iter::empty());
        assert_eq!(counts, TaskStatusCounts::default());
        assert_eq!(counts.total, 0);
    }

    #[test]
    fn ready_for_qa_is_tallied_in_its_own_bucket() {
        let counts = TaskStatusCounts::count(["ready-for-qa", "ready-for-qa", "todo"]);
        assert_eq!(counts.ready_for_qa, 2);
        assert_eq!(counts.todo, 1);
        assert_eq!(counts.other, 0);
        assert_eq!(counts.total, 3);
    }

    #[test]
    fn unrecognized_status_is_tallied_as_other() {
        let counts = TaskStatusCounts::count(["placeholder", "draft", "todo"]);
        assert_eq!(counts.other, 2);
        assert_eq!(counts.todo, 1);
        assert_eq!(counts.total, 3);
    }

    #[test]
    fn all_known_buckets_are_counted_independently() {
        let counts =
            TaskStatusCounts::count(["todo", "in-progress", "ready-for-qa", "done", "blocked"]);
        assert_eq!(counts.todo, 1);
        assert_eq!(counts.in_progress, 1);
        assert_eq!(counts.ready_for_qa, 1);
        assert_eq!(counts.done, 1);
        assert_eq!(counts.blocked, 1);
        assert_eq!(counts.other, 0);
        assert_eq!(counts.total, 5);
    }
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
pub struct StoryFields {
    pub id: String,
    pub kind: Option<String>,
    pub status_raw: String,
    pub status: Option<StoryStatus>,
    pub epic: Option<String>,
    pub sprint: Option<String>,
    pub priority_raw: Option<String>,
    pub priority: Option<i64>,
    pub story_points_raw: String,
    pub story_points: Option<i64>,
    pub assignee_raw: String,
    pub assignee: Option<String>,
    pub assignees: Vec<String>,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
    pub activated: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
}

impl StoryFields {
    pub fn from_frontmatter(frontmatter: &BTreeMap<String, String>) -> Self {
        let status_raw = frontmatter.get("status").cloned().unwrap_or_default();
        let story_points_raw = frontmatter.get("story_points").cloned().unwrap_or_default();
        let assignee_raw = frontmatter.get("assignee").cloned().unwrap_or_default();

        Self {
            id: frontmatter.get("id").cloned().unwrap_or_default(),
            kind: non_empty(frontmatter.get("type")),
            status: StoryStatus::parse(&status_raw),
            status_raw,
            epic: web_option(frontmatter.get("epic")),
            sprint: web_option(frontmatter.get("sprint")),
            priority_raw: frontmatter.get("priority").cloned(),
            priority: frontmatter
                .get("priority")
                .and_then(|value| value.trim().parse::<i64>().ok())
                .filter(|value| *value >= 0),
            story_points: story_points_raw.trim().parse::<i64>().ok(),
            story_points_raw,
            assignee: web_option(frontmatter.get("assignee")),
            assignees: parse_assignee_list(&assignee_raw),
            assignee_raw,
            work_started: non_empty(frontmatter.get("work_started")),
            work_done: non_empty(frontmatter.get("work_done")),
            activated: non_empty(frontmatter.get("activated")),
            created: non_empty(frontmatter.get("created")),
            updated: non_empty(frontmatter.get("updated")),
            planned_start: non_empty(frontmatter.get("planned_start")),
            planned_end: non_empty(frontmatter.get("planned_end")),
        }
    }

    pub fn sprint_frontmatter_value(&self) -> Option<&str> {
        self.sprint.as_deref()
    }
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn web_option(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty() && *value != "~" && *value != "null")
        .map(ToOwned::to_owned)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Story {
    pub file_path: PathBuf,
    pub relative_path: PathBuf,
    pub file_name: String,
    pub frontmatter: BTreeMap<String, String>,
    pub frontmatter_keys: BTreeSet<String>,
    pub fields: StoryFields,
    pub markdown: String,
    pub body: String,
    pub sprint_name: Option<String>,
    pub task_file: Option<TaskFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Epic {
    pub file_path: PathBuf,
    pub relative_path: PathBuf,
    pub file_name: String,
    pub frontmatter: BTreeMap<String, String>,
    pub frontmatter_keys: BTreeSet<String>,
    pub markdown: String,
    pub body: String,
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
    pub epic_id: Option<String>,
    pub epic_title: Option<String>,
    pub assignee: String,
    pub story_points: String,
    pub sprint: Option<String>,
    pub relative_path: PathBuf,
    pub task_summary: Option<TaskSummary>,
    pub task_count: usize,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpicOverview {
    pub id: String,
    pub title: String,
    pub status: String,
    pub phase: Option<String>,
    pub priority: Option<i64>,
    pub owner: Option<String>,
    pub milestone: Option<String>,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
    pub relative_path: PathBuf,
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
    pub wip_limit: Option<i64>,
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
pub struct CreateStoryInput {
    pub id: Option<String>,
    pub title: String,
    pub epic_id: String,
    pub status: String,
    pub sprint: String,
    pub story_points: String,
    pub assignee: Option<String>,
    pub priority: Option<String>,
    pub task_file: Option<String>,
    pub activated: Option<String>,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStoryResult {
    pub story_id: String,
    pub epic_id: String,
    pub sprint_name: Option<String>,
    pub story_path: PathBuf,
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
pub struct DeleteStoryResult {
    pub story_id: String,
    pub sprint_name: Option<String>,
    pub story_path: PathBuf,
    pub task_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMutationResult {
    pub story_id: String,
    pub task_id: String,
    pub task_file_path: PathBuf,
    pub task: Task,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListResult {
    pub story_id: String,
    pub task_file_path: Option<PathBuf>,
    pub tasks: Vec<Task>,
    pub task_summary: Option<TaskSummary>,
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
pub struct EpicUpdateResult {
    pub epic_id: String,
    pub epic_path: PathBuf,
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
    pub story_file_path: PathBuf,
    pub task_file_path: Option<PathBuf>,
    pub epic_id: Option<String>,
    pub epic_title: Option<String>,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
    pub story_statement: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub definition_of_done: Option<String>,
    pub notes_and_open_questions: Option<String>,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpicDetails {
    pub epic: EpicOverview,
    pub story_ids: Vec<String>,
    pub stories_by_status: BTreeMap<String, Vec<StoryOverview>>,
    pub child_stories: Vec<StoryOverview>,
    pub warnings: Vec<String>,
    pub body: String,
    pub business_context: Option<String>,
    pub business_value: Option<String>,
    pub scope: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub non_functional_requirements: Option<String>,
    pub dependencies: Option<String>,
    pub definition_of_done: Option<String>,
    pub notes_and_open_questions: Option<String>,
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
