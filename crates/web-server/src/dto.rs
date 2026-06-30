use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const BOARD_STATUSES: [&str; 6] = kanban_core::SPRINT_STATUS_DISPLAY_ORDER;

#[derive(Debug, Serialize)]
pub(crate) struct ApiError {
    pub(crate) error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitPullResponse {
    pub(crate) ok: bool,
    pub(crate) status: &'static str,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pulled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebTaskSummary {
    pub(crate) todo: usize,
    pub(crate) in_progress: usize,
    pub(crate) ready_for_qa: usize,
    pub(crate) done: usize,
    pub(crate) blocked: usize,
    pub(crate) total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WebTask {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) tags: Vec<String>,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Serialize)]
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
    pub(crate) frontmatter: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebStoryDetail {
    #[serde(flatten)]
    pub(crate) story: WebStory,
    pub(crate) body: String,
}

#[derive(Debug, Serialize)]
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
    pub(crate) stories_by_status: BTreeMap<String, Vec<WebStory>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WebEpic {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) phase: String,
    pub(crate) priority: Option<i64>,
    pub(crate) stories: Vec<WebStory>,
}

#[derive(Debug, Serialize)]
pub(crate) struct WebEpicDetail {
    #[serde(flatten)]
    pub(crate) epic: WebEpic,
    pub(crate) body: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhaseSummary {
    pub(crate) phase: String,
    pub(crate) done_points: i64,
    pub(crate) total_points: i64,
    pub(crate) done_stories: usize,
    pub(crate) total_stories: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectProgress {
    pub(crate) done_points: i64,
    pub(crate) total_points: i64,
    pub(crate) done_stories: usize,
    pub(crate) total_stories: usize,
    pub(crate) phases: Vec<PhaseSummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RepositorySnapshot {
    pub(crate) stories: Vec<WebStory>,
    pub(crate) epics: Vec<WebEpic>,
    pub(crate) sprints: Vec<WebSprint>,
    pub(crate) progress: ProjectProgress,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebTeamMember {
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigResponse {
    pub(crate) port: u16,
    pub(crate) host: String,
    pub(crate) style: String,
    pub(crate) version: String,
    pub(crate) branch: String,
    pub(crate) story_points: StoryPointsResponse,
}

#[derive(Debug, Serialize)]
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
    pub(crate) priority: i64,
}
