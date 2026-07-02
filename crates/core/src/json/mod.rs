//! Stable JSON envelope types for the `--format json` output mode.
//!
//! All types in this module derive `Serialize` and are intended to be
//! re-exported from `kanban_core` so they can be shared by the CLI and any
//! future web interface.

use std::path::Path;

use crate::util::normalize_status_alias;

mod dto_meta;
mod dto_validate;
mod dto_views;
mod dto_write;
mod envelope;
mod forecast;
mod report;

pub use dto_meta::*;
pub use dto_validate::*;
pub use dto_views::*;
pub use dto_write::*;
pub use envelope::*;
pub use forecast::*;
pub use report::*;

pub const SCHEMA_VERSION: u32 = 1;

/// Lowercase, trim, and replace spaces/underscores with hyphens.
pub fn slugify_status(status: &str) -> String {
    normalize_status_alias(status).replace([' ', '_'], "-")
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

/// Return `path` relative to `repo_root` as a forward-slashed string.
/// Falls back to the path as-is when `strip_prefix` fails (path already relative).
fn rel_to_root(repo_root: &Path, path: &Path) -> String {
    match path.strip_prefix(repo_root) {
        Ok(rel) => path_string(rel),
        Err(_) => path_string(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_status_maps_spaces_to_hyphens() {
        assert_eq!(slugify_status("In Progress"), "in-progress");
        assert_eq!(slugify_status("Ready for QA"), "ready-for-qa");
        assert_eq!(slugify_status("backlog"), "ready");
        assert_eq!(slugify_status("todo"), "todo");
    }

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
}
