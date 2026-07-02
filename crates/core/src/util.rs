#[allow(unused_imports)]
use crate::prelude::*;

pub(crate) fn current_timestamp_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string()
}

/// Normalize a status string by trimming, lowercasing, and mapping the
/// spaced human aliases (`to do`, `in progress`) and legacy synonyms to their
/// canonical forms. Unknown values pass through unchanged.
pub(crate) fn normalize_status_alias(status: &str) -> String {
    match status.trim().to_ascii_lowercase().as_str() {
        "backlog" => "ready".to_string(),
        "to do" => "todo".to_string(),
        "in progress" => "in-progress".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn current_git_assignee(repo_root: &Path) -> Result<String> {
    let name = git_config_value(repo_root, "user.name")?;
    let email = git_config_value(repo_root, "user.email")?;
    if name.is_empty() || email.is_empty() {
        bail!(
            "Git user.name and user.email must be configured before moving a story to in-progress."
        );
    }
    Ok(format!("{name} <{email}>"))
}

pub fn parse_assignee_list(assignee: &str) -> Vec<String> {
    assignee
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter(|entry| *entry != "~")
        .filter(|entry| !entry.eq_ignore_ascii_case("tbd"))
        .filter(|entry| !entry.eq_ignore_ascii_case("Name <email@example.com>"))
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn validate_assignee_override(assignee: &str) -> Result<String> {
    let pattern =
        Regex::new(r"^[^<>\s].*\s<[^<>\s@]+@[^<>\s@]+>$").expect("valid assignee validation regex");
    let assignees = parse_assignee_list(assignee);
    if assignees.is_empty() {
        bail!("Assignee must use the format `Name <email>`.");
    }

    for assignee in &assignees {
        if !pattern.is_match(assignee) {
            bail!("Assignee must use the format `Name <email>`.");
        }
    }

    Ok(assignees.join(", "))
}

pub(crate) fn normalize_story_assignee_value(assignee: &str) -> Result<String> {
    let trimmed = assignee.trim();
    if trimmed.is_empty() || trimmed == "~" || trimmed.eq_ignore_ascii_case("tbd") {
        return Ok(trimmed.to_string());
    }

    validate_assignee_override(trimmed)
}

pub(crate) fn git_config_value(repo_root: &Path, key: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("config")
        .arg(key)
        .output()
        .with_context(|| format!("read git config {key}"))?;
    if !output.status.success() {
        bail!("Git config {key} is not set.");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn first_weekday_after(date: NaiveDate, weekday: Weekday) -> NaiveDate {
    let mut current = date + Days::new(1);
    while current.weekday() != weekday {
        current = current + Days::new(1);
    }
    current
}

pub(crate) fn first_weekday_on_or_after(date: NaiveDate, weekday: Weekday) -> NaiveDate {
    let mut current = date;
    while current.weekday() != weekday {
        current = current + Days::new(1);
    }
    current
}

pub(crate) fn slugify_headline(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.trim().chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

pub(crate) fn parse_markdown_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.trim_matches('`').trim(), "%Y-%m-%d").ok()
}

pub(crate) fn date_only_timestamp(value: &str) -> Option<String> {
    let date = parse_markdown_date(value)?;
    let midnight = date.and_hms_opt(0, 0, 0)?;
    let local_midnight = Local.from_local_datetime(&midnight).earliest()?;
    Some(local_midnight.format("%Y-%m-%dT%H:%M:%S%z").to_string())
}

pub(crate) fn zulu_timestamp(value: &str) -> Option<String> {
    let timestamp = value.trim_matches('`').trim();
    if !timestamp.ends_with('Z') {
        return None;
    }

    let parsed = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
    Some(
        parsed
            .with_timezone(&Local)
            .format("%Y-%m-%dT%H:%M:%S%z")
            .to_string(),
    )
}

pub(crate) fn date_in_range(today: NaiveDate, start_date: NaiveDate, end_date: NaiveDate) -> bool {
    today >= start_date && today <= end_date
}

pub(crate) fn relative_path(repo_root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(repo_root).unwrap_or(path).to_path_buf()
}

/// Build a path from `from` to `to`, including `..` components when the target
/// sits outside `from`. Unlike `relative_path`, this is not repo-root-based and
/// preserves sibling-directory traversal for generated markdown links.
pub(crate) fn relative_path_from(from: &Path, to: &Path) -> PathBuf {
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let shared_prefix = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();
    for _ in shared_prefix..from_components.len() {
        relative.push("..");
    }
    for component in to_components.iter().skip(shared_prefix) {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

pub(crate) fn to_forward_slashes(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

/// Canonicalize the repository root, resolving symlinks so that a planted
/// symlinked backlog cannot spoof containment checks.
pub(crate) fn canonical_repo_root(repo_root: &Path) -> Result<PathBuf> {
    fs::canonicalize(repo_root)
        .with_context(|| format!("canonicalize repository root {}", repo_root.display()))
}

/// Ensure that `resolved` stays inside the canonicalized `repo_root`.
///
/// Used by every writer that joins a user-derived path (notably `task_file`
/// frontmatter and doctor `file_path` values) to the repository root, and by
/// the task-file read path so `kanban story show` never reads outside the
/// backlog root. Both `repo_root` and `resolved` are canonicalized so symlink
/// planting cannot escape the check.
///
/// When `resolved` does not yet exist (a new file about to be written), its
/// parent directory is canonicalized instead and the file name is rejoined,
/// which still detects `..` traversal and symlinked parents.
pub(crate) fn ensure_path_inside(repo_root: &Path, resolved: &Path) -> Result<PathBuf> {
    let canonical_root = canonical_repo_root(repo_root)?;
    let canonical_target = match fs::canonicalize(resolved) {
        Ok(path) => path,
        Err(_) => {
            let parent = resolved.parent().unwrap_or_else(|| Path::new(""));
            let file_name = resolved.file_name();
            let Some(file_name) = file_name else {
                bail!(
                    "Path {} has no file name component and cannot be written inside the backlog root.",
                    resolved.display()
                );
            };
            let canonical_parent = fs::canonicalize(parent)
                .with_context(|| format!("canonicalize parent {}", parent.display()))?;
            canonical_parent.join(file_name)
        }
    };

    if !canonical_target.starts_with(&canonical_root) {
        bail!(
            "Path {} resolves outside the backlog root {}.",
            canonical_target.display(),
            canonical_root.display()
        );
    }
    Ok(canonical_target)
}

/// Validate a `task_file` frontmatter value before it is joined to a story's
/// parent directory. The value must be a bare sibling file name: not absolute,
/// containing no path separators, and no `.` or `..` components. This is the
/// `invalid-task-file-path` validation rule shared by `validate` and the read
/// path.
pub(crate) fn validate_task_file_frontmatter_value(value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("task_file must not be empty when present.");
    }
    if trimmed.is_empty()
        || trimmed.contains(std::path::MAIN_SEPARATOR)
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains("/..")
        || trimmed.starts_with("../")
        || trimmed.starts_with("..\\")
        || Path::new(trimmed).is_absolute()
    {
        bail!(
            "task_file must be a sibling file name without `..`, path separators, or absolute paths; got {trimmed:?}."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_assignee_list, relative_path_from, slugify_headline};
    use std::path::{Path, PathBuf};

    #[test]
    fn parse_assignee_list_filters_placeholders() {
        assert!(parse_assignee_list("~").is_empty());
        assert!(parse_assignee_list("TBD").is_empty());
        assert!(parse_assignee_list("Name <email@example.com>").is_empty());
    }

    #[test]
    fn parse_assignee_list_keeps_valid_comma_separated_assignees() {
        assert_eq!(
            parse_assignee_list(
                "Alice Example <alice@example.com>, ~, TBD, Name <email@example.com>, Bob Example <bob@example.com>"
            ),
            vec![
                "Alice Example <alice@example.com>".to_string(),
                "Bob Example <bob@example.com>".to_string(),
            ]
        );
    }

    // Keep these cases aligned with web/shared/domain.test.ts.
    #[test]
    fn slugify_headline_matches_shared_domain_cases() {
        assert_eq!(slugify_headline("Foundation Sprint!"), "foundation-sprint");
        assert_eq!(slugify_headline("  Alpha   Beta  "), "alpha-beta");
        assert_eq!(slugify_headline("Roadmap_2026"), "roadmap-2026");
        assert_eq!(slugify_headline("---"), "");
    }

    #[test]
    fn relative_path_from_preserves_parent_traversal() {
        assert_eq!(
            relative_path_from(
                Path::new("delivery/sprints"),
                Path::new(
                    "delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/story.md"
                )
            ),
            PathBuf::from(
                "../backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/story.md"
            )
        );
    }

    #[test]
    fn relative_path_from_returns_dot_for_same_path() {
        assert_eq!(
            relative_path_from(Path::new("delivery/sprints"), Path::new("delivery/sprints")),
            PathBuf::from(".")
        );
    }
}
