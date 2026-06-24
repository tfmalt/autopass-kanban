#[allow(unused_imports)]
use crate::prelude::*;

pub(crate) const REQUIRED_STORY_FIELDS: [&str; 10] = [
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

pub const CANONICAL_STORY_STATUSES: [&str; 9] = [
    "draft",
    "backlog",
    "ready",
    "todo",
    "in-progress",
    "ready-for-qa",
    "blocked",
    "done",
    "dropped",
];

pub(crate) const TASK_HEADING_PATTERN: &str = r"(?m)^##\s+(TASK-[A-Z0-9-]+)\s+-\s+(.+)$";

pub(crate) const STORY_FILE_PREFIX: &str = "US-";

pub(crate) const EPIC_FILE_PREFIX: &str = "EP-";

pub(crate) const STORY_FILE_SUFFIX: &str = ".md";

pub(crate) const TASK_FILE_SUFFIX: &str = ".tasks.md";

pub(crate) const SPRINT_FILE_PATTERN: &str = r"^(S\d{3})\.([a-z0-9][a-z0-9-]*)\.md$";

pub(crate) const REQUIRED_SPRINT_README_FIELDS: [&str; 6] = [
    "sprint",
    "headline",
    "start_date",
    "end_date",
    "status",
    "wip_limit",
];

pub const SPRINT_STATUS_DISPLAY_ORDER: [&str; 5] =
    ["todo", "in-progress", "ready-for-qa", "done", "blocked"];

pub(crate) const STATUS_PROGRESSION: [&str; 6] = [
    "draft",
    "ready",
    "todo",
    "in-progress",
    "ready-for-qa",
    "done",
];

pub(crate) const SPRINT_STATUSES: [&str; 4] = ["planned", "active", "closed", "cancelled"];

pub(crate) const ROSTER_HEADING: &str = "## User Stories selected for sprint";

pub const CANONICAL_TASK_STATUSES: [&str; 4] = ["todo", "in-progress", "blocked", "done"];

pub(crate) fn status_rank(status: &str) -> Option<usize> {
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
