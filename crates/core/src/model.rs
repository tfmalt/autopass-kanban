#[allow(unused_imports)]
use crate::prelude::*;

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
    pub owner: Option<String>,
    pub milestone: Option<String>,
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
