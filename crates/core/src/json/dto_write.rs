use std::path::Path;

use serde::Serialize;

use crate::{
    CreateSprintResult, CreateStoryResult, DeleteStoryResult, EpicUpdateResult, MoveStoryResult,
    PlanStoryResult, RolloverResult, StoryUpdateResult, TaskListResult, TaskMutationResult,
    TaskSummary,
};

use super::{TaskDto, rel_to_root, slugify_status};

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

/// DTO for `story create` responses.
#[derive(Debug, Clone, Serialize)]
pub struct CreateStoryDto {
    pub story_id: String,
    pub epic_id: String,
    pub sprint_name: Option<String>,
    pub story_path: String,
}

impl CreateStoryDto {
    pub fn from_result(r: &CreateStoryResult, repo_root: &Path) -> Self {
        Self {
            story_id: r.story_id.clone(),
            epic_id: r.epic_id.clone(),
            sprint_name: r.sprint_name.clone(),
            story_path: rel_to_root(repo_root, &r.story_path),
        }
    }
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

/// DTO for `story delete` responses.
#[derive(Debug, Clone, Serialize)]
pub struct DeleteStoryDto {
    pub story_id: String,
    pub sprint_name: Option<String>,
    pub story_path: String,
    pub task_path: Option<String>,
}

impl DeleteStoryDto {
    pub fn from_result(r: &DeleteStoryResult, repo_root: &Path) -> Self {
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

/// DTO for `epic update` responses.
#[derive(Debug, Clone, Serialize)]
pub struct EpicUpdateDto {
    pub epic_id: String,
    pub epic_path: String,
    pub updated_fields: Vec<String>,
}

impl EpicUpdateDto {
    pub fn from_result(r: &EpicUpdateResult, repo_root: &Path) -> Self {
        Self {
            epic_id: r.epic_id.clone(),
            epic_path: rel_to_root(repo_root, &r.epic_path),
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

#[derive(Debug, Clone, Serialize)]
pub struct TaskShowDto {
    pub story_id: String,
    pub task_path: Option<String>,
    pub task_count: usize,
    pub tasks: Vec<TaskDto>,
    pub task_summary: Option<TaskSummary>,
}

impl TaskShowDto {
    pub fn from_result(r: &TaskListResult, repo_root: &Path) -> Self {
        Self {
            story_id: r.story_id.clone(),
            task_path: r
                .task_file_path
                .as_ref()
                .map(|path| rel_to_root(repo_root, path)),
            task_count: r.tasks.len(),
            tasks: r.tasks.iter().map(TaskDto::from_task).collect(),
            task_summary: r.task_summary.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn move_result_dto_emits_both_status_forms() {
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
        let task = crate::Task {
            id: "TASK-US-F1-001-001".to_string(),
            title: "Do something".to_string(),
            status: "todo".to_string(),
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
        assert_eq!(json["task"]["status"], "todo");
        assert_eq!(json["task"]["status_normalized"], "todo");
        assert_eq!(json["task"]["tags"][0], "cli");
    }

    #[test]
    fn task_show_dto_includes_task_list_and_summary() {
        let result = crate::TaskListResult {
            story_id: "US-F1-057".to_string(),
            task_file_path: Some(PathBuf::from("/repo/delivery/backlog/x/US-F1-057.tasks.md")),
            tasks: vec![crate::Task {
                id: "TASK-US-F1-057-001".to_string(),
                title: "First task".to_string(),
                status: "todo".to_string(),
                normalized_status: "todo".to_string(),
                tags: vec!["cli".to_string()],
                description: "desc".to_string(),
            }],
            task_summary: Some(crate::TaskSummary {
                todo: 1,
                in_progress: 0,
                blocked: 0,
                done: 0,
            }),
        };

        let dto = TaskShowDto::from_result(&result, Path::new("/repo"));
        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["story_id"], "US-F1-057");
        assert_eq!(json["task_path"], "delivery/backlog/x/US-F1-057.tasks.md");
        assert_eq!(json["task_count"], 1);
        assert_eq!(json["tasks"][0]["id"], "TASK-US-F1-057-001");
    }

    #[test]
    fn sprint_create_dto_relativizes_path() {
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
}
