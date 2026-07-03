use std::fmt;

use crate::util::normalize_status_alias;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum StoryStatus {
    Draft,
    Backlog,
    Ready,
    Planned,
    Todo,
    InProgress,
    ReadyForQa,
    Done,
    Blocked,
    Dropped,
}

impl StoryStatus {
    pub const ALL: [StoryStatus; 10] = [
        StoryStatus::Draft,
        StoryStatus::Backlog,
        StoryStatus::Ready,
        StoryStatus::Planned,
        StoryStatus::Todo,
        StoryStatus::InProgress,
        StoryStatus::ReadyForQa,
        StoryStatus::Done,
        StoryStatus::Blocked,
        StoryStatus::Dropped,
    ];

    pub const PROGRESSION: [StoryStatus; 8] = [
        StoryStatus::Draft,
        StoryStatus::Backlog,
        StoryStatus::Ready,
        StoryStatus::Planned,
        StoryStatus::Todo,
        StoryStatus::InProgress,
        StoryStatus::ReadyForQa,
        StoryStatus::Done,
    ];

    pub fn parse(status: &str) -> Option<Self> {
        let normalized = normalize_status_alias(status);
        match normalized.as_str() {
            "draft" => Some(StoryStatus::Draft),
            "ready" => Some(StoryStatus::Ready),
            "planned" => Some(StoryStatus::Planned),
            "todo" => Some(StoryStatus::Todo),
            "in-progress" => Some(StoryStatus::InProgress),
            "ready-for-qa" => Some(StoryStatus::ReadyForQa),
            "done" => Some(StoryStatus::Done),
            "blocked" => Some(StoryStatus::Blocked),
            "dropped" => Some(StoryStatus::Dropped),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            StoryStatus::Draft => "draft",
            StoryStatus::Backlog => "backlog",
            StoryStatus::Ready => "ready",
            StoryStatus::Planned => "planned",
            StoryStatus::Todo => "todo",
            StoryStatus::InProgress => "in-progress",
            StoryStatus::ReadyForQa => "ready-for-qa",
            StoryStatus::Done => "done",
            StoryStatus::Blocked => "blocked",
            StoryStatus::Dropped => "dropped",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, StoryStatus::Done | StoryStatus::Dropped)
    }

    pub const fn counts_toward_scope(self) -> bool {
        !matches!(self, StoryStatus::Dropped)
    }

    pub const fn board_bucket(self) -> StoryStatus {
        match self {
            StoryStatus::Dropped => StoryStatus::Done,
            status => status,
        }
    }

    pub fn rank(self) -> Option<usize> {
        Self::PROGRESSION.iter().position(|status| *status == self)
    }

    pub fn parse_is_terminal(status: &str) -> bool {
        Self::parse(status).is_some_and(Self::is_terminal)
    }

    pub fn parse_counts_toward_scope(status: &str) -> bool {
        Self::parse(status).is_none_or(Self::counts_toward_scope)
    }
}

impl fmt::Display for StoryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_aliases_using_core_normalization() {
        assert_eq!(
            StoryStatus::parse("In Progress"),
            Some(StoryStatus::InProgress)
        );
        assert_eq!(StoryStatus::parse("to do"), Some(StoryStatus::Todo));
        assert_eq!(StoryStatus::parse("backlog"), Some(StoryStatus::Ready));
        assert_eq!(StoryStatus::Backlog.as_str(), "backlog");
    }

    #[test]
    fn semantic_methods_cover_terminal_scope_and_bucket_rules() {
        assert!(StoryStatus::Done.is_terminal());
        assert!(StoryStatus::Dropped.is_terminal());
        assert!(StoryStatus::Done.counts_toward_scope());
        assert!(!StoryStatus::Dropped.counts_toward_scope());
        assert_eq!(StoryStatus::Dropped.board_bucket(), StoryStatus::Done);
    }
}
