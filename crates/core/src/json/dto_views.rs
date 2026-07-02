use std::collections::BTreeMap;

use serde::Serialize;

use crate::util::parse_assignee_list;
use crate::{
    BlockedWorkItem, Epic, EpicDetails, EpicOverview, PhaseOverview, SprintOverview, Story,
    StoryDetails, StoryOverview, Task, TaskSummary,
};

use super::{non_empty, parse_points, path_string, slugify_status};

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

/// DTO for an epic overview row.
#[derive(Debug, Clone, Serialize)]
pub struct EpicOverviewDto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub status_normalized: String,
    pub phase: Option<String>,
    pub owner: Option<String>,
    pub milestone: Option<String>,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
    pub path: String,
}

impl EpicOverviewDto {
    pub fn from_overview(o: &EpicOverview) -> Self {
        Self {
            id: o.id.clone(),
            title: o.title.clone(),
            status: o.status.clone(),
            status_normalized: slugify_status(&o.status),
            phase: o.phase.clone(),
            owner: o.owner.clone(),
            milestone: o.milestone.clone(),
            work_started: o.work_started.clone(),
            work_done: o.work_done.clone(),
            planned_start: o.planned_start.clone(),
            planned_end: o.planned_end.clone(),
            path: path_string(&o.relative_path),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EpicSectionsDto {
    pub business_context: Option<String>,
    pub business_value: Option<String>,
    pub scope: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub non_functional_requirements: Option<String>,
    pub dependencies: Option<String>,
    pub definition_of_done: Option<String>,
    pub notes_and_open_questions: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpicShowDto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub status_normalized: String,
    pub phase: Option<String>,
    pub owner: Option<String>,
    pub milestone: Option<String>,
    pub path: String,
    pub frontmatter: BTreeMap<String, String>,
    pub story_ids: Vec<String>,
    pub stories_by_status: BTreeMap<String, Vec<StoryOverviewDto>>,
    pub sections: EpicSectionsDto,
    pub body: String,
}

impl EpicShowDto {
    pub fn from_details(details: &EpicDetails, body: &str) -> Self {
        let mut stories_by_status = BTreeMap::new();
        for (status, stories) in &details.stories_by_status {
            stories_by_status.insert(
                slugify_status(status),
                stories
                    .iter()
                    .map(StoryOverviewDto::from_overview)
                    .collect(),
            );
        }

        Self {
            id: details.epic.id.clone(),
            title: details.epic.title.clone(),
            status: details.epic.status.clone(),
            status_normalized: slugify_status(&details.epic.status),
            phase: details.epic.phase.clone(),
            owner: details.epic.owner.clone(),
            milestone: details.epic.milestone.clone(),
            path: path_string(&details.epic.relative_path),
            frontmatter: BTreeMap::new(),
            story_ids: details.story_ids.clone(),
            stories_by_status,
            sections: EpicSectionsDto {
                business_context: details.business_context.clone(),
                business_value: details.business_value.clone(),
                scope: details.scope.clone(),
                acceptance_criteria: details.acceptance_criteria.clone(),
                non_functional_requirements: details.non_functional_requirements.clone(),
                dependencies: details.dependencies.clone(),
                definition_of_done: details.definition_of_done.clone(),
                notes_and_open_questions: details.notes_and_open_questions.clone(),
            },
            body: body.to_string(),
        }
    }

    pub fn from_details_and_source(details: &EpicDetails, source: &Epic) -> Self {
        Self {
            frontmatter: source.frontmatter.clone(),
            ..Self::from_details(details, &source.body)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

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
            planned_start: None,
            planned_end: None,
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
            planned_start: None,
            planned_end: None,
        };
        let dto = StoryOverviewDto::from_overview(&overview);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert!(json["story_points"].is_null());
        assert_eq!(json["assignee"], "A <a@b.no>");
        assert!(json["sprint"].is_null());
        assert!(json["task_summary"].is_null());
    }

    #[test]
    fn task_dto_maps_normalized_status() {
        let task = crate::Task {
            id: "TASK-US-F1-001-001".to_string(),
            title: "Do something".to_string(),
            status: "todo".to_string(),
            normalized_status: "todo".to_string(),
            tags: vec![],
            description: "desc".to_string(),
        };
        let dto = TaskDto::from_task(&task);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");
        assert_eq!(json["status"], "todo");
        assert_eq!(json["status_normalized"], "todo");
    }

    #[test]
    fn story_show_dto_carries_sections_and_raw_body() {
        let task = crate::Task {
            id: "TASK-US-F1-001-001".to_string(),
            title: "Some task".to_string(),
            status: "todo".to_string(),
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
            planned_start: None,
            planned_end: None,
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
        assert!(json["sections"]["definition_of_done"].is_null());
        assert_eq!(json["body"], "## body\ntext");
        assert_eq!(json["tasks"][0]["status_normalized"], "todo");
        assert_eq!(json["story_points"], 3);
    }

    #[test]
    fn story_show_dto_from_source_uses_source_frontmatter_and_body() {
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
            planned_start: None,
            planned_end: None,
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
            fields: crate::StoryFields::from_frontmatter(&fm),
            markdown: "---\nid: US-F1-001\nstatus: In Progress\n---\n\n## Body\nText".to_string(),
            body: "## Body\nText".to_string(),
            sprint_name: Some("S001".to_string()),
            task_file: None,
        };

        let dto = StoryShowDto::from_details_and_source(&details, &source);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");

        assert!(json["frontmatter"].is_object());
        assert_eq!(json["frontmatter"]["id"], "US-F1-001");
        assert_eq!(json["frontmatter"]["status"], "In Progress");
        assert_eq!(json["body"], "## Body\nText");
    }

    #[test]
    fn sprint_overview_dto_groups_by_normalized_status_with_flat_ids() {
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
            planned_start: None,
            planned_end: None,
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
            wip_limit: None,
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
        assert!(json["stories_by_status"]["in-progress"].is_array());

        let ids = json["story_ids"]
            .as_array()
            .expect("story_ids should be an array");
        let id_strings: Vec<&str> = ids.iter().filter_map(|v| v.as_str()).collect();
        assert!(id_strings.contains(&"US-F1-001"));
        assert!(id_strings.contains(&"US-F1-002"));

        let blocked = &json["blocked_work"][0];
        assert_eq!(blocked["story_id"], "US-F1-001");
        assert!(blocked["task_id"].is_null());
    }

    #[test]
    fn sprint_overview_dto_merges_slug_colliding_status_buckets() {
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
            planned_start: None,
            planned_end: None,
        };

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
            wip_limit: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };

        let dto = SprintOverviewDto::from_overview(&overview);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");

        let bucket = json["stories_by_status"]["in-progress"]
            .as_array()
            .expect("stories_by_status[in-progress] should be an array");
        assert_eq!(bucket.len(), 2);

        let ids = json["story_ids"]
            .as_array()
            .expect("story_ids should be an array");
        let id_strings: Vec<&str> = ids.iter().filter_map(|v| v.as_str()).collect();
        assert!(id_strings.contains(&"US-A"));
        assert!(id_strings.contains(&"US-B"));
    }
}
