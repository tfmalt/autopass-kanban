use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

pub(crate) const BOARD_STATUSES: [&str; 6] = kanban_core::SPRINT_STATUS_DISPLAY_ORDER;

#[derive(Debug, Serialize)]
pub(crate) struct ApiError {
    pub(crate) error: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(rename = "GitPullResponse")]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitPullResponse {
    pub(crate) ok: bool,
    #[ts(type = "\"success\" | \"error\" | \"in_progress\"")]
    pub(crate) status: &'static str,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub(crate) stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub(crate) stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub(crate) pulled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "TaskSummary")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebTaskSummary {
    pub(crate) todo: usize,
    pub(crate) in_progress: usize,
    pub(crate) ready_for_qa: usize,
    pub(crate) done: usize,
    pub(crate) blocked: usize,
    pub(crate) total: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "Task")]
pub(crate) struct WebTask {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) tags: Vec<String>,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "Story")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebStory {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) phase: Option<String>,
    pub(crate) epic: Option<String>,
    pub(crate) sprint: Option<String>,
    pub(crate) priority: Option<i64>,
    pub(crate) story_points: Option<i64>,
    pub(crate) assignee: Option<String>,
    pub(crate) assignees: Vec<String>,
    pub(crate) work_started: Option<String>,
    pub(crate) work_done: Option<String>,
    pub(crate) activated: Option<String>,
    pub(crate) created: Option<String>,
    pub(crate) updated: Option<String>,
    pub(crate) relative_path: String,
    pub(crate) tasks: Vec<WebTask>,
    pub(crate) task_summary: WebTaskSummary,
    #[ts(type = "Record<string, string>")]
    pub(crate) frontmatter: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(rename = "StoryDetail")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebStoryDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub(crate) story: WebStory,
    pub(crate) body: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(rename = "Sprint")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebSprint {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) headline: String,
    pub(crate) goal: Option<String>,
    pub(crate) start_date: Option<String>,
    pub(crate) end_date: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) wip_limit: Option<i64>,
    #[ts(type = "Record<StoryStatus, Story[]>")]
    pub(crate) stories_by_status: BTreeMap<String, Vec<WebStory>>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "Epic")]
pub(crate) struct WebEpic {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) phase: String,
    pub(crate) priority: Option<i64>,
    pub(crate) planned_start: Option<String>,
    pub(crate) planned_end: Option<String>,
    pub(crate) work_started: Option<String>,
    pub(crate) work_done: Option<String>,
    pub(crate) stories: Vec<WebStory>,
}

#[derive(Debug, Serialize, TS)]
#[ts(rename = "EpicDetail")]
pub(crate) struct WebEpicDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub(crate) epic: WebEpic,
    pub(crate) body: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhaseSummary {
    pub(crate) phase: String,
    pub(crate) done_points: i64,
    pub(crate) total_points: i64,
    pub(crate) done_stories: usize,
    pub(crate) total_stories: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectProgress {
    pub(crate) done_points: i64,
    pub(crate) total_points: i64,
    pub(crate) done_stories: usize,
    pub(crate) total_stories: usize,
    pub(crate) phases: Vec<PhaseSummary>,
}

#[derive(Debug, Serialize, TS)]
pub(crate) struct RepositorySnapshot {
    pub(crate) stories: Vec<WebStory>,
    pub(crate) epics: Vec<WebEpic>,
    pub(crate) sprints: Vec<WebSprint>,
    pub(crate) progress: ProjectProgress,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "ReportEstimate")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebReportEstimate {
    pub(crate) story_id: String,
    pub(crate) est_hours: Option<f64>,
    pub(crate) est_start: Option<String>,
    pub(crate) est_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "ReportWbsRow")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebReportWbsRow {
    pub(crate) kind: String,
    pub(crate) wbs: String,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) milestone: String,
    pub(crate) period: String,
    pub(crate) priority: String,
    pub(crate) status: String,
    pub(crate) points: Option<i64>,
    pub(crate) est_hours: Option<f64>,
    pub(crate) start_date: Option<String>,
    pub(crate) end_date: Option<String>,
    pub(crate) notes: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "ReportPhaseRow")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebReportPhaseRow {
    pub(crate) phase: String,
    pub(crate) title: String,
    pub(crate) period: String,
    pub(crate) milestone: String,
    pub(crate) epics: usize,
    pub(crate) stories: usize,
    pub(crate) total: i64,
    pub(crate) done: i64,
    pub(crate) wip: i64,
    pub(crate) remaining: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "ReportSprintProjection")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebReportSprintProjection {
    pub(crate) name: String,
    pub(crate) start_date: String,
    pub(crate) end_date: String,
    pub(crate) planned_points: Option<i64>,
    pub(crate) delivered_points: Option<i64>,
    pub(crate) rate: Option<f64>,
    pub(crate) remaining: Option<i64>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "ReportProgress")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebReportProgress {
    pub(crate) done_points: i64,
    pub(crate) total_points: i64,
    pub(crate) done_stories: usize,
    pub(crate) total_stories: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "ReportDashboard")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebReportDashboard {
    pub(crate) generated_at: String,
    pub(crate) daily_avg: f64,
    pub(crate) throughput_source: String,
    pub(crate) hours_per_point: f64,
    pub(crate) remaining_points: i64,
    pub(crate) progress: WebReportProgress,
    pub(crate) forecast: crate::metrics::Forecast,
    pub(crate) estimates: Vec<WebReportEstimate>,
    pub(crate) wbs_rows: Vec<WebReportWbsRow>,
    pub(crate) phase_rows: Vec<WebReportPhaseRow>,
    pub(crate) sprint_rows: Vec<WebReportSprintProjection>,
}

impl From<kanban_core::ReportEstimateDto> for WebReportEstimate {
    fn from(dto: kanban_core::ReportEstimateDto) -> Self {
        Self {
            story_id: dto.story_id,
            est_hours: dto.est_hours,
            est_start: dto.est_start,
            est_end: dto.est_end,
        }
    }
}

impl From<kanban_core::ReportWbsRowDto> for WebReportWbsRow {
    fn from(dto: kanban_core::ReportWbsRowDto) -> Self {
        Self {
            kind: dto.kind,
            wbs: dto.wbs,
            id: dto.id,
            title: dto.title,
            milestone: dto.milestone,
            period: dto.period,
            priority: dto.priority,
            status: dto.status,
            points: dto.points,
            est_hours: dto.est_hours,
            start_date: dto.start_date,
            end_date: dto.end_date,
            notes: dto.notes,
        }
    }
}

impl From<kanban_core::ReportPhaseRowDto> for WebReportPhaseRow {
    fn from(dto: kanban_core::ReportPhaseRowDto) -> Self {
        Self {
            phase: dto.phase,
            title: dto.title,
            period: dto.period,
            milestone: dto.milestone,
            epics: dto.epics,
            stories: dto.stories,
            total: dto.total,
            done: dto.done,
            wip: dto.wip,
            remaining: dto.remaining,
        }
    }
}

impl From<kanban_core::ReportSprintProjectionDto> for WebReportSprintProjection {
    fn from(dto: kanban_core::ReportSprintProjectionDto) -> Self {
        Self {
            name: dto.name,
            start_date: dto.start_date,
            end_date: dto.end_date,
            planned_points: dto.planned_points,
            delivered_points: dto.delivered_points,
            rate: dto.rate,
            remaining: dto.remaining,
            status: dto.status,
        }
    }
}

impl From<kanban_core::ReportProgressDto> for WebReportProgress {
    fn from(dto: kanban_core::ReportProgressDto) -> Self {
        Self {
            done_points: dto.done_points,
            total_points: dto.total_points,
            done_stories: dto.done_stories,
            total_stories: dto.total_stories,
        }
    }
}

impl From<kanban_core::ReportDashboardDto> for WebReportDashboard {
    fn from(dto: kanban_core::ReportDashboardDto) -> Self {
        Self {
            generated_at: dto.generated_at,
            daily_avg: dto.daily_avg,
            throughput_source: dto.throughput_source,
            hours_per_point: dto.hours_per_point,
            remaining_points: dto.remaining_points,
            progress: dto.progress.into(),
            forecast: dto.forecast.into(),
            estimates: dto.estimates.into_iter().map(Into::into).collect(),
            wbs_rows: dto.wbs_rows.into_iter().map(Into::into).collect(),
            phase_rows: dto.phase_rows.into_iter().map(Into::into).collect(),
            sprint_rows: dto.sprint_rows.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(rename = "TeamMember")]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebTeamMember {
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub(crate) avatar_url: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigResponse {
    pub(crate) port: u16,
    pub(crate) host: String,
    pub(crate) style: String,
    pub(crate) version: String,
    pub(crate) branch: String,
    pub(crate) story_points: StoryPointsResponse,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoryPointsResponse {
    pub(crate) allowed_values: Vec<String>,
    pub(crate) aliases: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoveInput {
    pub(crate) status: String,
    pub(crate) assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlanInput {
    pub(crate) sprint: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateTaskInput {
    pub(crate) status: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateBodyInput {
    pub(crate) body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateStoryFieldsInput {
    pub(crate) assignee: Option<String>,
    pub(crate) sprint: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) story_points: Option<Value>,
    pub(crate) priority: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEpicFieldsInput {
    pub(crate) priority: Option<i64>,
    pub(crate) planned_start: Option<String>,
    pub(crate) planned_end: Option<String>,
    pub(crate) work_started: Option<String>,
    pub(crate) work_done: Option<String>,
}
