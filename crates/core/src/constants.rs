use crate::StoryStatus;
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

pub const CANONICAL_STORY_STATUSES: [&str; 10] = story_status_strings(StoryStatus::ALL);

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

pub const SPRINT_STATUS_DISPLAY_ORDER: [&str; 6] = [
    "planned",
    "todo",
    "in-progress",
    "ready-for-qa",
    "done",
    "blocked",
];

pub(crate) const SPRINT_STATUSES: [&str; 4] = ["planned", "active", "closed", "cancelled"];

pub(crate) const ROSTER_HEADING: &str = "## User Stories selected for sprint";

pub const CANONICAL_TASK_STATUSES: [&str; 4] = ["todo", "in-progress", "blocked", "done"];

const fn story_status_strings<const N: usize>(statuses: [StoryStatus; N]) -> [&'static str; N] {
    let mut out = [""; N];
    let mut index = 0;
    while index < N {
        out[index] = statuses[index].as_str();
        index += 1;
    }
    out
}

pub(crate) fn status_rank(status: &str) -> Option<usize> {
    StoryStatus::parse(status).and_then(StoryStatus::rank)
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
