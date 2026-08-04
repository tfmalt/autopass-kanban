//! Lazily compiled shared regular expressions.
//!
//! Every pattern here used to be compiled with `Regex::new` at each call site,
//! inside per-story and per-task-file loops. Compilation dominated the residual
//! cost of `validate` and `doctor` once the configuration blowup was removed.
//! These statics are immutable after first use, so they add no shared mutable
//! state.

use std::sync::LazyLock;

use regex::Regex;

use crate::constants::TASK_HEADING_PATTERN;

/// `## TASK-XXX - Title` headings inside a task file.
pub(crate) static TASK_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(TASK_HEADING_PATTERN).expect("valid task heading regex"));

/// Local ISO 8601 with a numeric timezone offset, e.g. `2026-05-28T14:05:54+0200`.
pub(crate) static LOCAL_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{4}$").expect("valid timestamp regex")
});

/// `YYYY-MM-DD`.
pub(crate) static MARKDOWN_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("valid date regex"));

/// `SNNN.headline.md` sprint file names.
pub(crate) static SPRINT_FILE_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(crate::constants::SPRINT_FILE_PATTERN).expect("valid sprint file regex")
});

/// `Name <email>` assignee validation.
pub(crate) static ASSIGNEE_STRICT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^<>\s].*\s<[^<>\s@]+@[^<>\s@]+>$").expect("valid assignee validation regex")
});

/// `Name <email>` assignee capture for roster rendering.
pub(crate) static ASSIGNEE_CAPTURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?P<name>[^<]+?)\s*<(?P<email>[^>]+)>").expect("valid assignee parse regex")
});
