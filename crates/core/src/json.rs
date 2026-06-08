//! Stable JSON envelope types for the `--format json` output mode.
//!
//! All types in this module derive `Serialize` and are intended to be
//! re-exported from `kanban_core` so they can be shared by the CLI and any
//! future web interface.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::util::parse_assignee_list;
use crate::{
    BlockedWorkItem, CompletionItem, ConfigInitResult, ConfigSetResult, CreateSprintResult,
    DoctorFinding, MoveStoryResult, PhaseOverview, PlanStoryResult, RolloverResult, SprintOverview,
    Story, StoryDetails, StoryOverview, StoryUpdateResult, Task, TaskMutationResult, TaskSummary,
    ValidationReport,
};

pub const SCHEMA_VERSION: u32 = 1;

/// Top-level status of a JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Ok,
    Warning,
    Error,
}

/// Machine-readable error code embedded in `KanbanErrorBody`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KanbanErrorCode {
    NotInitialized,
    StoryNotFound,
    SprintNotFound,
    PhaseNotFound,
    InvalidStatus,
    InvalidArgument,
    ConfigKeyNotFound,
    IoError,
    ParseError,
    Internal,
}

impl KanbanErrorCode {
    /// Heuristic classification of an `anyhow` error into a `KanbanErrorCode`.
    pub fn classify(error: &anyhow::Error) -> Self {
        let msg = error.to_string().to_lowercase();
        if msg.contains("kanban init") || msg.contains(".kanban") {
            KanbanErrorCode::NotInitialized
        } else if msg.contains("unsupported story status")
            || msg.contains("unsupported task status")
        {
            KanbanErrorCode::InvalidStatus
        } else if msg.contains("sprint not found") {
            KanbanErrorCode::SprintNotFound
        } else if msg.contains("story not found") {
            KanbanErrorCode::StoryNotFound
        } else if msg.contains("frontmatter") || msg.contains("parse") {
            KanbanErrorCode::ParseError
        } else if msg.contains("no such file")
            || msg.contains("permission denied")
            || msg.contains("i/o")
        {
            KanbanErrorCode::IoError
        } else {
            KanbanErrorCode::Internal
        }
    }
}

/// Error body embedded in a JSON envelope when `status` is `"error"`.
#[derive(Debug, Clone, Serialize)]
pub struct KanbanErrorBody {
    pub code: KanbanErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl KanbanErrorBody {
    pub fn new(code: KanbanErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn from_anyhow(error: &anyhow::Error) -> Self {
        Self::new(KanbanErrorCode::classify(error), error.to_string())
    }
}

/// Top-level JSON envelope emitted by `--format json`.
#[derive(Debug, Serialize)]
pub struct JsonEnvelope<T: Serialize> {
    pub status: ResultStatus,
    pub kind: &'static str,
    pub schema_version: u32,
    pub data: Option<T>,
    pub error: Option<KanbanErrorBody>,
}

impl<T: Serialize> JsonEnvelope<T> {
    pub fn ok(kind: &'static str, data: T) -> Self {
        Self {
            status: ResultStatus::Ok,
            kind,
            schema_version: SCHEMA_VERSION,
            data: Some(data),
            error: None,
        }
    }

    pub fn warning(kind: &'static str, data: T) -> Self {
        Self {
            status: ResultStatus::Warning,
            kind,
            schema_version: SCHEMA_VERSION,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(kind: &'static str, body: KanbanErrorBody) -> Self {
        Self {
            status: ResultStatus::Error,
            kind,
            schema_version: SCHEMA_VERSION,
            data: None,
            error: Some(body),
        }
    }

    /// Returns the process exit code for this envelope.
    pub fn exit_code(&self) -> i32 {
        match self.status {
            ResultStatus::Ok => 0,
            ResultStatus::Warning => 2,
            ResultStatus::Error => 1,
        }
    }
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// Placeholder data type for error-only envelopes where the command has no DTO.
#[derive(Debug, Clone, Serialize)]
pub struct NoData;

/// DTO for `config get` responses.
#[derive(Debug, Serialize)]
pub struct ConfigGetDto {
    pub key: String,
    pub value: String,
}

/// DTO for `init` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigInitDto {
    pub repo_root: String,
    pub config_dir: String,
    pub created_files: Vec<String>,
    pub created_count: usize,
}

impl ConfigInitDto {
    pub fn from_result(r: &ConfigInitResult) -> Self {
        let created_files: Vec<String> = r.created_files.iter().map(|p| path_string(p)).collect();
        let created_count = created_files.len();
        Self {
            repo_root: path_string(&r.repo_root),
            config_dir: path_string(&r.config_dir),
            created_files,
            created_count,
        }
    }
}

/// DTO for `config set` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigSetDto {
    pub key: String,
    pub value: String,
    pub file_path: String,
}

impl ConfigSetDto {
    pub fn from_result(r: &ConfigSetResult) -> Self {
        Self {
            key: r.key.clone(),
            value: r.value.clone(),
            file_path: path_string(&r.file_path),
        }
    }
}

/// DTO for `completion` responses in JSON mode.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionDto {
    pub target: String,
    pub content_type: String,
    pub content: String,
}

/// DTO item for `list-ids` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ListIdItemDto {
    pub value: String,
    pub description: Option<String>,
}

impl ListIdItemDto {
    pub fn value(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: None,
        }
    }

    pub fn from_completion_item(item: &CompletionItem) -> Self {
        Self {
            value: item.value.clone(),
            description: non_empty(&item.description),
        }
    }
}

/// DTO for hidden `list-ids` responses.
#[derive(Debug, Clone, Serialize)]
pub struct ListIdsDto {
    pub kind: String,
    pub count: usize,
    pub items: Vec<ListIdItemDto>,
}

impl ListIdsDto {
    pub fn new(kind: impl Into<String>, items: Vec<ListIdItemDto>) -> Self {
        let count = items.len();
        Self {
            kind: kind.into(),
            count,
            items,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Lowercase, trim, and replace spaces/underscores with hyphens.
pub fn slugify_status(status: &str) -> String {
    status.trim().to_ascii_lowercase().replace([' ', '_'], "-")
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_points(raw: &str) -> Option<i64> {
    raw.trim().parse::<i64>().ok()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

// ── Shared DTOs ───────────────────────────────────────────────────────────────

/// DTO for a single story overview row, used in story list and sprint views.
#[derive(Debug, Clone, Serialize)]
pub struct StoryOverviewDto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub status_normalized: String,
    pub assignee: Option<String>,
    pub assignees: Vec<String>,
    pub story_points: Option<i64>,
    pub sprint: Option<String>,
    pub path: String,
    pub task_summary: Option<TaskSummary>,
    pub task_count: usize,
}

impl StoryOverviewDto {
    pub fn from_overview(o: &StoryOverview) -> Self {
        Self {
            id: o.id.clone(),
            title: o.title.clone(),
            status: o.status.clone(),
            status_normalized: slugify_status(&o.status),
            assignee: non_empty(&o.assignee),
            assignees: parse_assignee_list(&o.assignee),
            story_points: parse_points(&o.story_points),
            sprint: o.sprint.clone(),
            path: path_string(&o.relative_path),
            task_summary: o.task_summary.clone(),
            task_count: o.task_count,
        }
    }
}

/// DTO for a single task, used in story show views.
#[derive(Debug, Clone, Serialize)]
pub struct TaskDto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub status_normalized: String,
    pub tags: Vec<String>,
    pub description: String,
}

impl TaskDto {
    pub fn from_task(t: &Task) -> Self {
        Self {
            id: t.id.clone(),
            title: t.title.clone(),
            status: t.status.clone(),
            status_normalized: t.normalized_status.clone(),
            tags: t.tags.clone(),
            description: t.description.clone(),
        }
    }
}

/// Section content extracted from a story's markdown body.
#[derive(Debug, Clone, Serialize)]
pub struct StorySectionsDto {
    pub story_statement: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub definition_of_done: Option<String>,
    pub notes_and_open_questions: Option<String>,
}

/// DTO for a full story detail view (`story show`).
#[derive(Debug, Clone, Serialize)]
pub struct StoryShowDto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub status_normalized: String,
    pub assignee: Option<String>,
    pub assignees: Vec<String>,
    pub story_points: Option<i64>,
    pub sprint: Option<String>,
    pub path: String,
    pub task_path: Option<String>,
    pub frontmatter: BTreeMap<String, String>,
    pub sections: StorySectionsDto,
    pub body: String,
    pub tasks: Vec<TaskDto>,
    pub task_summary: Option<TaskSummary>,
}

impl StoryShowDto {
    /// Build from a `StoryDetails`, using `body` as the raw markdown body,
    /// with an empty frontmatter map. Use [`StoryShowDto::from_details_and_source`]
    /// to also populate frontmatter from the raw parsed story in one step.
    pub fn from_details(details: &StoryDetails, body: &str) -> Self {
        let o = &details.story;
        Self {
            id: o.id.clone(),
            title: o.title.clone(),
            status: o.status.clone(),
            status_normalized: slugify_status(&o.status),
            assignee: non_empty(&o.assignee),
            assignees: parse_assignee_list(&o.assignee),
            story_points: parse_points(&o.story_points),
            sprint: o.sprint.clone(),
            path: path_string(&o.relative_path),
            task_path: details.task_file_path.as_deref().map(path_string),
            frontmatter: BTreeMap::new(),
            sections: StorySectionsDto {
                story_statement: details.story_statement.clone(),
                acceptance_criteria: details.acceptance_criteria.clone(),
                definition_of_done: details.definition_of_done.clone(),
                notes_and_open_questions: details.notes_and_open_questions.clone(),
            },
            body: body.to_string(),
            tasks: details.tasks.iter().map(TaskDto::from_task).collect(),
            task_summary: o.task_summary.clone(),
        }
    }

    /// Build a complete story DTO from details plus the raw source story
    /// (frontmatter + body), in one step.
    pub fn from_details_and_source(details: &StoryDetails, source: &Story) -> Self {
        Self {
            frontmatter: source.frontmatter.clone(),
            ..Self::from_details(details, &source.body)
        }
    }
}

/// DTO for a story list response (`story list`).
#[derive(Debug, Clone, Serialize)]
pub struct StoryListDto {
    pub scope: String,
    pub count: usize,
    pub stories: Vec<StoryOverviewDto>,
}

impl StoryListDto {
    pub fn new(scope: impl Into<String>, stories: &[StoryOverview]) -> Self {
        let dtos: Vec<StoryOverviewDto> = stories
            .iter()
            .map(StoryOverviewDto::from_overview)
            .collect();
        let count = dtos.len();
        Self {
            scope: scope.into(),
            count,
            stories: dtos,
        }
    }
}

// ── Sprint DTOs ───────────────────────────────────────────────────────────────

/// DTO for a single blocked-work item in a sprint overview.
#[derive(Debug, Clone, Serialize)]
pub struct BlockedWorkDto {
    pub story_id: String,
    pub story_title: String,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
}

impl BlockedWorkDto {
    fn from_item(item: &BlockedWorkItem) -> Self {
        Self {
            story_id: item.story_id.clone(),
            story_title: item.story_title.clone(),
            task_id: item.task_id.clone(),
            task_title: item.task_title.clone(),
        }
    }
}

/// DTO for a full sprint overview (`sprint current` / `sprint show`).
#[derive(Debug, Clone, Serialize)]
pub struct SprintOverviewDto {
    pub sprint_name: String,
    pub headline: String,
    pub sprint_goal: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub path: String,
    pub readme_status: Option<String>,
    /// Flat list of story IDs in iteration order (across all statuses).
    pub story_ids: Vec<String>,
    pub stories_by_status: BTreeMap<String, Vec<StoryOverviewDto>>,
    pub blocked_work: Vec<BlockedWorkDto>,
    pub warnings: Vec<String>,
}

impl SprintOverviewDto {
    pub fn from_overview(o: &SprintOverview) -> Self {
        let mut story_ids: Vec<String> = Vec::new();
        let mut stories_by_status: BTreeMap<String, Vec<StoryOverviewDto>> = BTreeMap::new();

        for (status, stories) in &o.stories_by_status {
            let slug = slugify_status(status);
            for story in stories {
                story_ids.push(story.id.clone());
            }
            let dtos: Vec<StoryOverviewDto> = stories
                .iter()
                .map(StoryOverviewDto::from_overview)
                .collect();
            stories_by_status.entry(slug).or_default().extend(dtos);
        }

        Self {
            sprint_name: o.sprint_name.clone(),
            headline: o.headline.clone(),
            sprint_goal: o.sprint_goal.clone(),
            start_date: o.start_date.clone(),
            end_date: o.end_date.clone(),
            path: path_string(&o.readme_path),
            readme_status: o.readme_status.clone(),
            story_ids,
            stories_by_status,
            blocked_work: o
                .blocked_work
                .iter()
                .map(BlockedWorkDto::from_item)
                .collect(),
            warnings: o.warnings.clone(),
        }
    }
}

/// DTO for a single sprint in a sprint list.
#[derive(Debug, Clone, Serialize)]
pub struct SprintListItemDto {
    pub sprint_name: String,
    pub headline: String,
    pub start_date: String,
    pub end_date: String,
    pub path: String,
    pub readme_status: Option<String>,
    pub is_current: bool,
}

/// DTO for a sprint list response (`sprint list`).
#[derive(Debug, Clone, Serialize)]
pub struct SprintListDto {
    pub count: usize,
    pub sprints: Vec<SprintListItemDto>,
}

impl SprintListDto {
    pub fn new(sprints: &[SprintOverview], current_name: Option<&str>) -> Self {
        let items: Vec<SprintListItemDto> = sprints
            .iter()
            .map(|o| SprintListItemDto {
                sprint_name: o.sprint_name.clone(),
                headline: o.headline.clone(),
                start_date: o.start_date.clone(),
                end_date: o.end_date.clone(),
                path: path_string(&o.readme_path),
                readme_status: o.readme_status.clone(),
                is_current: current_name == Some(o.sprint_name.as_str()),
            })
            .collect();
        let count = items.len();
        Self {
            count,
            sprints: items,
        }
    }
}

// ── Phase DTOs ────────────────────────────────────────────────────────────────

/// DTO for a phase backlog view (`phase show`).
#[derive(Debug, Clone, Serialize)]
pub struct PhaseShowDto {
    pub phase: String,
    pub count: usize,
    pub stories: Vec<StoryOverviewDto>,
}

impl PhaseShowDto {
    pub fn from_overview(o: &PhaseOverview) -> Self {
        let stories: Vec<StoryOverviewDto> = o
            .stories
            .iter()
            .map(StoryOverviewDto::from_overview)
            .collect();
        let count = stories.len();
        Self {
            phase: o.phase.clone(),
            count,
            stories,
        }
    }
}

// ── Validate / Doctor DTOs ────────────────────────────────────────────────────

/// DTO for a single validation issue, used in `validate` responses.
#[derive(Debug, Clone, Serialize)]
pub struct IssueDto {
    pub path: String,
    pub rule: String,
    pub message: String,
}

/// DTO for a `validate` response.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateDto {
    pub valid: bool,
    pub story_count: usize,
    pub issue_count: usize,
    pub issues: Vec<IssueDto>,
}

impl ValidateDto {
    pub fn from_report(report: &ValidationReport, repo_root: &Path) -> Self {
        let valid = report.issues.is_empty();
        let issues: Vec<IssueDto> = report
            .issues
            .iter()
            .map(|i| IssueDto {
                path: rel_to_root(repo_root, &i.file_path),
                rule: i.rule.clone(),
                message: i.message.clone(),
            })
            .collect();
        Self {
            valid,
            story_count: report.stories.len(),
            issue_count: issues.len(),
            issues,
        }
    }
}

/// DTO for a single doctor finding.
#[derive(Debug, Clone, Serialize)]
pub struct FindingDto {
    pub severity: String,
    pub scope: String,
    pub message: String,
}

/// Summary counts of doctor findings by severity.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DoctorSummary {
    pub error: usize,
    pub warning: usize,
    pub info: usize,
}

/// DTO for a `doctor` response.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorDto {
    pub healthy: bool,
    pub findings: Vec<FindingDto>,
    pub summary: DoctorSummary,
}

impl DoctorDto {
    pub fn from_findings(findings: &[DoctorFinding]) -> Self {
        let mut summary = DoctorSummary::default();
        for f in findings {
            match f.severity.to_ascii_lowercase().as_str() {
                "error" => summary.error += 1,
                "warning" => summary.warning += 1,
                _ => summary.info += 1,
            }
        }
        let healthy = findings.is_empty();
        Self {
            healthy,
            findings: findings
                .iter()
                .map(|f| FindingDto {
                    severity: f.severity.clone(),
                    scope: f.scope.clone(),
                    message: f.message.clone(),
                })
                .collect(),
            summary,
        }
    }
}

// ── Write-result DTOs ─────────────────────────────────────────────────────────

/// DTO for `story move` responses.
#[derive(Debug, Clone, Serialize)]
pub struct MoveStoryDto {
    pub story_id: String,
    pub sprint_name: String,
    pub from_status: String,
    pub from_status_normalized: String,
    pub to_status: String,
    pub to_status_normalized: String,
    pub story_path: String,
    pub task_path: Option<String>,
}

impl MoveStoryDto {
    pub fn from_result(r: &MoveStoryResult, repo_root: &Path) -> Self {
        Self {
            story_id: r.story_id.clone(),
            sprint_name: r.sprint_name.clone(),
            from_status: r.from_status.clone(),
            from_status_normalized: slugify_status(&r.from_status),
            to_status: r.to_status.clone(),
            to_status_normalized: slugify_status(&r.to_status),
            story_path: rel_to_root(repo_root, &r.story_path),
            task_path: r.task_path.as_deref().map(|p| rel_to_root(repo_root, p)),
        }
    }
}

/// DTO for `story plan` responses.
#[derive(Debug, Clone, Serialize)]
pub struct PlanStoryDto {
    pub story_id: String,
    pub sprint_name: String,
    pub story_path: String,
    pub task_path: Option<String>,
}

impl PlanStoryDto {
    pub fn from_result(r: &PlanStoryResult, repo_root: &Path) -> Self {
        Self {
            story_id: r.story_id.clone(),
            sprint_name: r.sprint_name.clone(),
            story_path: rel_to_root(repo_root, &r.story_path),
            task_path: r.task_path.as_deref().map(|p| rel_to_root(repo_root, p)),
        }
    }
}

/// DTO for `story update` responses.
#[derive(Debug, Clone, Serialize)]
pub struct StoryUpdateDto {
    pub story_id: String,
    pub story_path: String,
    pub updated_fields: Vec<String>,
}

impl StoryUpdateDto {
    pub fn from_result(r: &StoryUpdateResult, repo_root: &Path) -> Self {
        Self {
            story_id: r.story_id.clone(),
            story_path: rel_to_root(repo_root, &r.story_path),
            updated_fields: r.updated_fields.clone(),
        }
    }
}

/// DTO for `task add` / `task update` responses.
#[derive(Debug, Clone, Serialize)]
pub struct TaskMutationDto {
    pub story_id: String,
    pub task_id: String,
    pub task_path: String,
    pub task: TaskDto,
}

impl TaskMutationDto {
    pub fn from_result(r: &TaskMutationResult, repo_root: &Path) -> Self {
        Self {
            story_id: r.story_id.clone(),
            task_id: r.task_id.clone(),
            task_path: rel_to_root(repo_root, &r.task_file_path),
            task: TaskDto::from_task(&r.task),
        }
    }
}

/// DTO for `sprint create` responses.
#[derive(Debug, Clone, Serialize)]
pub struct SprintCreateDto {
    pub sprint_name: String,
    pub path: String,
}

impl SprintCreateDto {
    pub fn from_result(r: &CreateSprintResult, repo_root: &Path) -> Self {
        Self {
            sprint_name: r.sprint_name.clone(),
            path: rel_to_root(repo_root, &r.sprint_path),
        }
    }
}

/// DTO for `sprint rollover` responses.
#[derive(Debug, Clone, Serialize)]
pub struct SprintRolloverDto {
    pub from_sprint: String,
    pub to_sprint: String,
    pub created_next_sprint: bool,
    pub completed_story_ids: Vec<String>,
    pub carried_story_ids: Vec<String>,
}

impl SprintRolloverDto {
    pub fn from_result(r: &RolloverResult) -> Self {
        Self {
            from_sprint: r.from_sprint.clone(),
            to_sprint: r.to_sprint.clone(),
            created_next_sprint: r.created_next_sprint,
            completed_story_ids: r.completed_story_ids.clone(),
            carried_story_ids: r.carried_story_ids.clone(),
        }
    }
}

/// DTO for `sprint sync` responses.
#[derive(Debug, Clone, Serialize)]
pub struct SprintSyncDto {
    pub changed_sprints: Vec<String>,
    pub count: usize,
}

impl SprintSyncDto {
    pub fn from_changed(changed: Vec<String>) -> Self {
        let count = changed.len();
        Self {
            changed_sprints: changed,
            count,
        }
    }
}

/// Parse a raw config JSON string into a `serde_json::Value`.
pub fn config_show_value(config_json: &str) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(config_json)
}

/// Return `path` relative to `repo_root` as a forward-slashed string.
/// Falls back to the path as-is when `strip_prefix` fails (path already relative).
fn rel_to_root(repo_root: &Path, path: &Path) -> String {
    match path.strip_prefix(repo_root) {
        Ok(rel) => path_string(rel),
        Err(_) => path_string(path),
    }
}

// ── Report DTOs ───────────────────────────────────────────────────────────────

fn phase_from_story_id(id: &str) -> String {
    // US-F1-001 → "F1", US-F2-010 → "F2"
    id.split('-').nth(1).unwrap_or("unknown").to_string()
}

/// Per-story row in the WBS report.
#[derive(Debug, Clone, Serialize)]
pub struct ReportStoryDto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub story_points: Option<i64>,
    pub sprint: Option<String>,
    pub epic_id: Option<String>,
    pub epic_title: Option<String>,
    pub phase: String,
    pub path: String,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
}

impl ReportStoryDto {
    pub fn from_overview(o: &StoryOverview) -> Self {
        Self {
            phase: phase_from_story_id(&o.id),
            id: o.id.clone(),
            title: o.title.clone(),
            status: o.status.clone(),
            story_points: parse_points(&o.story_points),
            sprint: o.sprint.clone(),
            epic_id: o.epic_id.clone(),
            epic_title: o.epic_title.clone(),
            path: path_string(&o.relative_path),
            work_started: o.work_started.clone(),
            work_done: o.work_done.clone(),
        }
    }
}

/// Per-sprint burndown row in the WBS report.
#[derive(Debug, Clone, Serialize)]
pub struct ReportSprintDto {
    pub sprint_name: String,
    pub start_date: String,
    pub end_date: String,
    pub is_current: bool,
    pub is_past: bool,
    pub planned_points: i64,
    pub delivered_points: i64,
    pub story_ids: Vec<String>,
}

/// Per-phase summary row.
#[derive(Debug, Clone, Serialize)]
pub struct ReportPhaseDto {
    pub phase: String,
    pub story_count: usize,
    pub points_total: i64,
    pub points_done: i64,
    pub points_in_progress: i64,
    pub points_remaining: i64,
}

/// Velocity and prognosis summary.
#[derive(Debug, Clone, Serialize)]
pub struct ReportVelocityDto {
    pub completed_sprint_count: usize,
    pub avg_points_per_sprint: f64,
    pub remaining_points: i64,
    pub estimated_sprints_remaining: Option<f64>,
    pub sprint_duration_weeks: u32,
}

/// Top-level payload for `kanban report wbs --format json`.
#[derive(Debug, Clone, Serialize)]
pub struct ReportWbsDto {
    pub generated_at: String,
    pub stories: Vec<ReportStoryDto>,
    pub sprints: Vec<ReportSprintDto>,
    pub phases: Vec<ReportPhaseDto>,
    pub velocity: ReportVelocityDto,
}

impl ReportWbsDto {
    pub fn build(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        current_sprint_name: Option<&str>,
    ) -> Self {
        use chrono::Local;

        let today = Local::now().date_naive();
        let generated_at = Local::now().to_rfc3339();

        // Build story lookup: sprint_name → (planned_pts, done_pts, ids)
        let mut sprint_stats: BTreeMap<String, (i64, i64, Vec<String>)> = BTreeMap::new();
        for story in stories {
            if let Some(ref sprint) = story.sprint {
                let pts = parse_points(&story.story_points).unwrap_or(0);
                let entry = sprint_stats.entry(sprint.clone()).or_default();
                entry.0 += pts;
                if story.status.eq_ignore_ascii_case("done") {
                    entry.1 += pts;
                }
                entry.2.push(story.id.clone());
            }
        }

        // Build sprint DTOs with computed stats
        let sprint_dtos: Vec<ReportSprintDto> = sprints
            .iter()
            .map(|s| {
                let end =
                    chrono::NaiveDate::parse_from_str(&s.end_date, "%Y-%m-%d").unwrap_or(today);
                let is_past = end < today;
                let is_current = Some(s.sprint_name.as_str()) == current_sprint_name;
                let (planned, done, ids) = sprint_stats
                    .get(&s.sprint_name)
                    .cloned()
                    .unwrap_or_default();
                ReportSprintDto {
                    sprint_name: s.sprint_name.clone(),
                    start_date: s.start_date.clone(),
                    end_date: s.end_date.clone(),
                    is_current,
                    is_past,
                    planned_points: planned,
                    delivered_points: done,
                    story_ids: ids,
                }
            })
            .collect();

        // Phase summaries
        let mut phase_map: BTreeMap<String, (usize, i64, i64, i64, i64)> = BTreeMap::new();
        for story in stories {
            let phase = phase_from_story_id(&story.id);
            let pts = parse_points(&story.story_points).unwrap_or(0);
            let e = phase_map.entry(phase).or_default();
            e.0 += 1;
            e.1 += pts;
            let status = story.status.to_ascii_lowercase();
            if status == "done" {
                e.2 += pts;
            } else if status == "in-progress" || status == "ready-for-qa" {
                e.3 += pts;
            } else {
                e.4 += pts;
            }
        }
        let phase_dtos: Vec<ReportPhaseDto> = phase_map
            .into_iter()
            .map(|(phase, (count, total, done, wip, rem))| ReportPhaseDto {
                phase,
                story_count: count,
                points_total: total,
                points_done: done,
                points_in_progress: wip,
                points_remaining: rem,
            })
            .collect();

        // Velocity: use past sprints that had stories
        let past_with_stories: Vec<&ReportSprintDto> = sprint_dtos
            .iter()
            .filter(|s| s.is_past && s.planned_points > 0)
            .collect();
        let completed_count = past_with_stories.len();
        let total_delivered: i64 = past_with_stories.iter().map(|s| s.delivered_points).sum();
        let avg_velocity = if completed_count > 0 {
            total_delivered as f64 / completed_count as f64
        } else {
            0.0
        };

        let remaining: i64 = stories
            .iter()
            .filter(|s| !s.status.eq_ignore_ascii_case("done"))
            .map(|s| parse_points(&s.story_points).unwrap_or(0))
            .sum();

        let est_sprints = if avg_velocity > 0.0 {
            Some(remaining as f64 / avg_velocity)
        } else {
            None
        };

        // Infer typical sprint duration from sprint history
        let sprint_duration_weeks = sprint_dtos
            .first()
            .and_then(|s| {
                let start = chrono::NaiveDate::parse_from_str(&s.start_date, "%Y-%m-%d").ok()?;
                let end = chrono::NaiveDate::parse_from_str(&s.end_date, "%Y-%m-%d").ok()?;
                Some(((end - start).num_days() as f64 / 7.0).round() as u32)
            })
            .unwrap_or(2);

        ReportWbsDto {
            generated_at,
            stories: stories.iter().map(ReportStoryDto::from_overview).collect(),
            sprints: sprint_dtos,
            phases: phase_dtos,
            velocity: ReportVelocityDto {
                completed_sprint_count: completed_count,
                avg_points_per_sprint: avg_velocity,
                remaining_points: remaining,
                estimated_sprints_remaining: est_sprints,
                sprint_duration_weeks,
            },
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn task_summary_serializes_in_progress_with_hyphen() {
        let summary = crate::TaskSummary {
            todo: 2,
            in_progress: 1,
            blocked: 0,
            done: 4,
        };
        let json = serde_json::to_value(&summary).expect("serialization should succeed");
        assert_eq!(json["todo"], 2);
        assert_eq!(json["in-progress"], 1);
        assert_eq!(json["blocked"], 0);
        assert_eq!(json["done"], 4);
    }

    #[test]
    fn slugify_status_maps_spaces_to_hyphens() {
        assert_eq!(slugify_status("In Progress"), "in-progress");
        assert_eq!(slugify_status("Ready for QA"), "ready-for-qa");
        assert_eq!(slugify_status("todo"), "todo");
    }

    #[test]
    fn story_overview_dto_types_points_and_normalizes_status() {
        let overview = crate::StoryOverview {
            id: "US-F1-001".to_string(),
            title: "Cluster".to_string(),
            status: "In Progress".to_string(),
            epic_id: None,
            epic_title: None,
            assignee: String::new(),
            story_points: "3".to_string(),
            sprint: Some("S001".to_string()),
            relative_path: PathBuf::from("delivery/backlog/x/US-F1-001-cluster.md"),
            task_summary: Some(crate::TaskSummary {
                todo: 1,
                in_progress: 0,
                blocked: 0,
                done: 0,
            }),
            task_count: 1,
            work_started: None,
            work_done: None,
        };
        let dto = StoryOverviewDto::from_overview(&overview);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["status"], "In Progress");
        assert_eq!(json["status_normalized"], "in-progress");
        assert_eq!(json["story_points"], 3);
        assert!(json["assignee"].is_null());
        assert_eq!(json["sprint"], "S001");
        assert_eq!(json["path"], "delivery/backlog/x/US-F1-001-cluster.md");
    }

    #[test]
    fn story_points_is_null_when_unparseable() {
        let overview = crate::StoryOverview {
            id: "US-F1-002".to_string(),
            title: "Test".to_string(),
            status: "todo".to_string(),
            epic_id: None,
            epic_title: None,
            assignee: "A <a@b.no>".to_string(),
            story_points: String::new(),
            sprint: None,
            relative_path: PathBuf::from("delivery/backlog/x/US-F1-002-test.md"),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: None,
        };
        let dto = StoryOverviewDto::from_overview(&overview);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert!(json["story_points"].is_null());
        assert_eq!(json["assignee"], "A <a@b.no>");
        assert!(json["sprint"].is_null());
        assert!(json["task_summary"].is_null());
    }

    #[test]
    fn ok_envelope_serializes_with_all_keys() {
        let env = JsonEnvelope::ok(
            "config.get",
            ConfigGetDto {
                key: "paths.backlog".to_string(),
                value: "delivery/backlog".to_string(),
            },
        );
        let json = serde_json::to_value(&env).expect("serialization should succeed");
        assert_eq!(json["status"], "ok");
        assert_eq!(json["kind"], "config.get");
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["data"]["key"], "paths.backlog");
        assert_eq!(json["data"]["value"], "delivery/backlog");
        assert!(json["error"].is_null());
    }

    #[test]
    fn error_envelope_has_null_data_and_populated_error() {
        let env: JsonEnvelope<ConfigGetDto> = JsonEnvelope::error(
            "config.get",
            KanbanErrorBody::new(KanbanErrorCode::ConfigKeyNotFound, "no such key"),
        );
        let json = serde_json::to_value(&env).expect("serialization should succeed");
        assert_eq!(json["status"], "error");
        assert!(json["data"].is_null());
        assert_eq!(json["error"]["code"], "config_key_not_found");
        assert_eq!(json["error"]["message"], "no such key");
        assert!(json["error"]["details"].is_null());
    }

    #[test]
    fn error_code_serializes_as_snake_case() {
        let value = serde_json::to_value(KanbanErrorCode::StoryNotFound)
            .expect("serialization should succeed");
        assert_eq!(
            value,
            serde_json::Value::String("story_not_found".to_string())
        );
    }

    #[test]
    fn task_dto_maps_normalized_status() {
        let task = crate::Task {
            id: "TASK-US-F1-001-001".to_string(),
            title: "Do something".to_string(),
            status: "To Do".to_string(),
            normalized_status: "todo".to_string(),
            tags: vec![],
            description: "desc".to_string(),
        };
        let dto = TaskDto::from_task(&task);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["status"], "To Do");
        assert_eq!(json["status_normalized"], "todo");
    }

    #[test]
    fn story_show_dto_carries_sections_and_raw_body() {
        use std::path::PathBuf;

        let task = crate::Task {
            id: "TASK-US-F1-001-001".to_string(),
            title: "Some task".to_string(),
            status: "To Do".to_string(),
            normalized_status: "todo".to_string(),
            tags: vec![],
            description: "desc".to_string(),
        };
        let overview = crate::StoryOverview {
            id: "US-F1-001".to_string(),
            title: "Cluster".to_string(),
            status: "In Progress".to_string(),
            epic_id: None,
            epic_title: None,
            assignee: String::new(),
            story_points: "3".to_string(),
            sprint: Some("S001".to_string()),
            relative_path: PathBuf::from("delivery/backlog/x/US-F1-001.md"),
            task_summary: None,
            task_count: 1,
            work_started: None,
            work_done: None,
        };
        let details = crate::StoryDetails {
            story: overview,
            story_file_path: PathBuf::from("delivery/backlog/x/US-F1-001.md"),
            task_file_path: Some(PathBuf::from("delivery/backlog/x/US-F1-001.tasks.md")),
            epic_id: None,
            epic_title: None,
            work_started: None,
            work_done: None,
            story_statement: Some("As a user, I want something.".to_string()),
            acceptance_criteria: Some("Given ... then ...".to_string()),
            definition_of_done: None,
            notes_and_open_questions: None,
            tasks: vec![task],
        };

        let dto = StoryShowDto::from_details(&details, "## body\ntext");
        let json = serde_json::to_value(&dto).expect("serialization should succeed");

        assert_eq!(json["id"], "US-F1-001");
        assert_eq!(json["status_normalized"], "in-progress");
        assert_eq!(json["task_path"], "delivery/backlog/x/US-F1-001.tasks.md");
        assert_eq!(
            json["sections"]["story_statement"],
            "As a user, I want something."
        );
        assert!(
            json["sections"]["definition_of_done"].is_null(),
            "definition_of_done should be null when None"
        );
        assert_eq!(json["body"], "## body\ntext");
        assert_eq!(json["tasks"][0]["status_normalized"], "todo");
        assert_eq!(json["story_points"], 3);
    }

    #[test]
    fn story_show_dto_from_source_uses_source_frontmatter_and_body() {
        use std::collections::BTreeSet;
        use std::path::PathBuf;

        let overview = crate::StoryOverview {
            id: "US-F1-001".to_string(),
            title: "Cluster".to_string(),
            status: "In Progress".to_string(),
            epic_id: None,
            epic_title: None,
            assignee: String::new(),
            story_points: "3".to_string(),
            sprint: Some("S001".to_string()),
            relative_path: PathBuf::from("delivery/backlog/x/US-F1-001.md"),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: None,
        };
        let details = crate::StoryDetails {
            story: overview,
            story_file_path: PathBuf::from("delivery/backlog/x/US-F1-001.md"),
            task_file_path: None,
            epic_id: None,
            epic_title: None,
            work_started: None,
            work_done: None,
            story_statement: None,
            acceptance_criteria: None,
            definition_of_done: None,
            notes_and_open_questions: None,
            tasks: vec![],
        };

        let mut fm = BTreeMap::new();
        fm.insert("id".to_string(), "US-F1-001".to_string());
        fm.insert("status".to_string(), "In Progress".to_string());
        let source = crate::Story {
            file_path: PathBuf::from("delivery/backlog/x/US-F1-001.md"),
            relative_path: PathBuf::from("delivery/backlog/x/US-F1-001.md"),
            file_name: "US-F1-001.md".to_string(),
            frontmatter: fm.clone(),
            frontmatter_keys: BTreeSet::from(["id".to_string(), "status".to_string()]),
            markdown: "---\nid: US-F1-001\nstatus: In Progress\n---\n\n## Body\nText".to_string(),
            body: "## Body\nText".to_string(),
            sprint_name: Some("S001".to_string()),
            task_file: None,
        };

        let dto = StoryShowDto::from_details_and_source(&details, &source);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");

        assert!(
            json["frontmatter"].is_object(),
            "frontmatter should be an object"
        );
        assert_eq!(
            json["frontmatter"]["id"], "US-F1-001",
            "frontmatter id should come from source"
        );
        assert_eq!(
            json["frontmatter"]["status"], "In Progress",
            "frontmatter status should come from source"
        );
        assert_eq!(
            json["body"], "## Body\nText",
            "body should come from source"
        );
    }

    #[test]
    fn sprint_overview_dto_groups_by_normalized_status_with_flat_ids() {
        use std::path::PathBuf;

        let make_story = |id: &str, status: &str| crate::StoryOverview {
            id: id.to_string(),
            title: format!("Story {id}"),
            status: status.to_string(),
            epic_id: None,
            epic_title: None,
            assignee: String::new(),
            story_points: "2".to_string(),
            sprint: Some("S001.foundation".to_string()),
            relative_path: PathBuf::from(format!(
                "delivery/backlog/phase-1/01.infra/{id}-story.md"
            )),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: None,
        };

        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![make_story("US-F1-001", "In Progress")],
        );
        stories_by_status.insert("todo".to_string(), vec![make_story("US-F1-002", "Todo")]);

        let overview = crate::SprintOverview {
            sprint_name: "S001".to_string(),
            headline: "foundation".to_string(),
            sprint_goal: Some("Build the base".to_string()),
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-12".to_string(),
            readme_path: PathBuf::from("delivery/sprints/S001.foundation.md"),
            readme_status: Some("active".to_string()),
            stories_by_status,
            blocked_work: vec![crate::BlockedWorkItem {
                story_id: "US-F1-001".to_string(),
                story_title: "Story US-F1-001".to_string(),
                task_id: None,
                task_title: None,
            }],
            warnings: vec!["w".to_string()],
        };

        let dto = SprintOverviewDto::from_overview(&overview);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");

        assert_eq!(json["sprint_name"], "S001");
        assert_eq!(json["path"], "delivery/sprints/S001.foundation.md");
        assert_eq!(json["readme_status"], "active");
        assert!(
            json["stories_by_status"]["in-progress"].is_array(),
            "stories_by_status[in-progress] should be an array"
        );

        let ids = json["story_ids"]
            .as_array()
            .expect("story_ids should be an array");
        let id_strings: Vec<&str> = ids.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            id_strings.contains(&"US-F1-001"),
            "story_ids should contain US-F1-001; got: {id_strings:?}"
        );
        assert!(
            id_strings.contains(&"US-F1-002"),
            "story_ids should contain US-F1-002; got: {id_strings:?}"
        );

        let blocked = &json["blocked_work"][0];
        assert_eq!(blocked["story_id"], "US-F1-001");
        assert!(
            blocked["task_id"].is_null(),
            "task_id should be null when None"
        );
    }

    #[test]
    fn sprint_overview_dto_merges_slug_colliding_status_buckets() {
        use std::path::PathBuf;

        let make_story = |id: &str, status: &str| crate::StoryOverview {
            id: id.to_string(),
            title: format!("Story {id}"),
            status: status.to_string(),
            epic_id: None,
            epic_title: None,
            assignee: String::new(),
            story_points: "1".to_string(),
            sprint: Some("S001".to_string()),
            relative_path: PathBuf::from(format!(
                "delivery/backlog/phase-1/01.infra/{id}-story.md"
            )),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: None,
        };

        // Two source keys that slugify to the same slug "in-progress"
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![make_story("US-A", "in-progress")],
        );
        stories_by_status.insert(
            "In Progress".to_string(),
            vec![make_story("US-B", "In Progress")],
        );

        let overview = crate::SprintOverview {
            sprint_name: "S001".to_string(),
            headline: "foundation".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-12".to_string(),
            readme_path: PathBuf::from("delivery/sprints/S001.foundation.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };

        let dto = SprintOverviewDto::from_overview(&overview);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");

        // Both stories should be merged into the "in-progress" bucket (length 2)
        let bucket = json["stories_by_status"]["in-progress"]
            .as_array()
            .expect("stories_by_status[in-progress] should be an array");
        assert_eq!(
            bucket.len(),
            2,
            "slug-colliding buckets should be merged, not overwritten; got {} stories",
            bucket.len()
        );

        // story_ids should contain both US-A and US-B
        let ids = json["story_ids"]
            .as_array()
            .expect("story_ids should be an array");
        let id_strings: Vec<&str> = ids.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            id_strings.contains(&"US-A"),
            "story_ids should contain US-A; got: {id_strings:?}"
        );
        assert!(
            id_strings.contains(&"US-B"),
            "story_ids should contain US-B; got: {id_strings:?}"
        );
    }

    #[test]
    fn validate_dto_reports_counts_and_validity() {
        use std::path::PathBuf;

        let report = crate::ValidationReport {
            repo_root: PathBuf::from("/repo"),
            stories: vec![],
            issues: vec![crate::ValidationIssue {
                file_path: PathBuf::from("/repo/delivery/backlog/x/US-F1-009.md"),
                rule: "missing_frontmatter_field".to_string(),
                message: "missing status".to_string(),
            }],
        };
        let dto = ValidateDto::from_report(&report, std::path::Path::new("/repo"));
        assert!(
            !dto.valid,
            "dto.valid should be false when there are issues"
        );
        assert_eq!(dto.issue_count, 1);
        assert_eq!(dto.story_count, 0);
        assert_eq!(dto.issues[0].rule, "missing_frontmatter_field");
        assert_eq!(
            dto.issues[0].path, "delivery/backlog/x/US-F1-009.md",
            "path should be relativized to repo root"
        );

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["valid"], false);
        assert_eq!(json["issue_count"], 1);
        assert_eq!(json["story_count"], 0);
        assert_eq!(json["issues"][0]["rule"], "missing_frontmatter_field");
        assert_eq!(json["issues"][0]["path"], "delivery/backlog/x/US-F1-009.md");
    }

    #[test]
    fn doctor_dto_summarizes_findings_by_severity() {
        let findings = vec![
            crate::DoctorFinding {
                severity: "warning".to_string(),
                scope: "US-F1-001".to_string(),
                message: "story has no sprint".to_string(),
            },
            crate::DoctorFinding {
                severity: "warning".to_string(),
                scope: "US-F1-002".to_string(),
                message: "story has no sprint".to_string(),
            },
        ];
        let dto = DoctorDto::from_findings(&findings);
        assert!(
            !dto.healthy,
            "dto.healthy should be false when findings exist"
        );
        assert_eq!(dto.summary.warning, 2);
        assert_eq!(dto.summary.error, 0);
        assert_eq!(dto.summary.info, 0);
        assert_eq!(dto.findings[0].scope, "US-F1-001");

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["healthy"], false);
        assert_eq!(json["summary"]["warning"], 2);
        assert_eq!(json["summary"]["error"], 0);
        assert_eq!(json["findings"][0]["scope"], "US-F1-001");
    }

    #[test]
    fn move_result_dto_emits_both_status_forms() {
        use std::path::PathBuf;

        let r = crate::MoveStoryResult {
            story_id: "US-F1-001".to_string(),
            sprint_name: "S001.foundation".to_string(),
            from_status: "Todo".to_string(),
            to_status: "In Progress".to_string(),
            story_path: PathBuf::from("/repo/delivery/backlog/x/US-F1-001.md"),
            task_path: Some(PathBuf::from("/repo/delivery/backlog/x/US-F1-001.tasks.md")),
        };
        let dto = MoveStoryDto::from_result(&r, std::path::Path::new("/repo"));
        assert_eq!(dto.from_status, "Todo");
        assert_eq!(dto.from_status_normalized, "todo");
        assert_eq!(dto.to_status, "In Progress");
        assert_eq!(dto.to_status_normalized, "in-progress");
        assert_eq!(dto.story_path, "delivery/backlog/x/US-F1-001.md");
        assert_eq!(
            dto.task_path.as_deref(),
            Some("delivery/backlog/x/US-F1-001.tasks.md")
        );

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["from_status"], "Todo");
        assert_eq!(json["from_status_normalized"], "todo");
        assert_eq!(json["to_status_normalized"], "in-progress");
        assert_eq!(json["story_path"], "delivery/backlog/x/US-F1-001.md");
        assert_eq!(json["task_path"], "delivery/backlog/x/US-F1-001.tasks.md");
    }

    #[test]
    fn plan_story_dto_maps_paths() {
        use std::path::PathBuf;

        let r = crate::PlanStoryResult {
            story_id: "US-F2-001".to_string(),
            sprint_name: "S002.delivery".to_string(),
            story_path: PathBuf::from("/repo/delivery/backlog/p/US-F2-001.md"),
            task_path: None,
        };
        let dto = PlanStoryDto::from_result(&r, std::path::Path::new("/repo"));
        assert_eq!(dto.story_id, "US-F2-001");
        assert_eq!(dto.sprint_name, "S002.delivery");
        assert_eq!(dto.story_path, "delivery/backlog/p/US-F2-001.md");
        assert!(dto.task_path.is_none());

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["story_id"], "US-F2-001");
        assert!(json["task_path"].is_null());
    }

    #[test]
    fn task_mutation_dto_includes_task_and_path() {
        use std::path::PathBuf;

        let task = crate::Task {
            id: "TASK-US-F1-001-001".to_string(),
            title: "Do something".to_string(),
            status: "To Do".to_string(),
            normalized_status: "todo".to_string(),
            tags: vec!["cli".to_string()],
            description: "desc".to_string(),
        };
        let r = crate::TaskMutationResult {
            story_id: "US-F1-001".to_string(),
            task_id: "TASK-US-F1-001-001".to_string(),
            task_file_path: PathBuf::from("/repo/delivery/backlog/x/US-F1-001.tasks.md"),
            task: task.clone(),
        };
        let dto = TaskMutationDto::from_result(&r, std::path::Path::new("/repo"));
        assert_eq!(dto.task_id, "TASK-US-F1-001-001");
        assert_eq!(dto.task_path, "delivery/backlog/x/US-F1-001.tasks.md");
        assert_eq!(dto.task.status_normalized, "todo");

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["task"]["status"], "To Do");
        assert_eq!(json["task"]["status_normalized"], "todo");
        assert_eq!(json["task"]["tags"][0], "cli");
    }

    #[test]
    fn sprint_create_dto_relativizes_path() {
        use std::path::PathBuf;

        let r = crate::CreateSprintResult {
            sprint_name: "S003.testing".to_string(),
            sprint_path: PathBuf::from("/repo/delivery/sprints/S003.testing.md"),
        };
        let dto = SprintCreateDto::from_result(&r, std::path::Path::new("/repo"));
        assert_eq!(dto.sprint_name, "S003.testing");
        assert_eq!(dto.path, "delivery/sprints/S003.testing.md");

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["sprint_name"], "S003.testing");
        assert_eq!(json["path"], "delivery/sprints/S003.testing.md");
    }

    #[test]
    fn sprint_rollover_dto_from_result() {
        let r = crate::RolloverResult {
            from_sprint: "S001.foundation".to_string(),
            to_sprint: "S002.delivery".to_string(),
            created_next_sprint: true,
            completed_story_ids: vec!["US-F1-001".to_string()],
            carried_story_ids: vec!["US-F1-002".to_string(), "US-F1-003".to_string()],
        };
        let dto = SprintRolloverDto::from_result(&r);
        assert_eq!(dto.from_sprint, "S001.foundation");
        assert_eq!(dto.to_sprint, "S002.delivery");
        assert!(dto.created_next_sprint);
        assert_eq!(dto.completed_story_ids.len(), 1);
        assert_eq!(dto.carried_story_ids.len(), 2);

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["from_sprint"], "S001.foundation");
        assert_eq!(json["created_next_sprint"], true);
        assert_eq!(json["carried_story_ids"][1], "US-F1-003");
    }

    #[test]
    fn sprint_sync_dto_reports_changed() {
        let dto = SprintSyncDto::from_changed(vec!["S001.foundation".to_string()]);
        assert_eq!(dto.count, 1);
        assert_eq!(dto.changed_sprints[0], "S001.foundation");

        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["count"], 1);
        assert_eq!(json["changed_sprints"][0], "S001.foundation");
    }

    #[test]
    fn classify_maps_story_and_sprint_not_found() {
        let story_err = anyhow::anyhow!("Story not found: US-F1-999");
        assert_eq!(
            KanbanErrorCode::classify(&story_err),
            KanbanErrorCode::StoryNotFound,
            "plain 'Story not found' should map to StoryNotFound"
        );

        let sprint_err = anyhow::anyhow!("Sprint not found: S009");
        assert_eq!(
            KanbanErrorCode::classify(&sprint_err),
            KanbanErrorCode::SprintNotFound,
            "'Sprint not found' should map to SprintNotFound"
        );

        let sprint_story_err = anyhow::anyhow!("Sprint story not found: US-F1-001");
        assert_eq!(
            KanbanErrorCode::classify(&sprint_story_err),
            KanbanErrorCode::StoryNotFound,
            "'Sprint story not found' should map to StoryNotFound"
        );
    }
}
