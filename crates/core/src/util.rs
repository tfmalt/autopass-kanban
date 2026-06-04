use crate::markdown::*;
#[allow(unused_imports)]
use crate::prelude::*;

pub(crate) fn current_timestamp_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%z").to_string()
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

pub(crate) fn validate_assignee_override(assignee: &str) -> Result<String> {
    let trimmed = assignee.trim();
    let pattern =
        Regex::new(r"^[^<>\s].*\s<[^<>\s@]+@[^<>\s@]+>$").expect("valid assignee validation regex");
    if pattern.is_match(trimmed) {
        Ok(trimmed.to_string())
    } else {
        bail!("Assignee must use the format `Name <email>`.");
    }
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

pub(crate) fn date_only_timestamp_from_file(
    file_path: &Path,
    field_name: &str,
) -> Result<Option<String>> {
    let markdown = fs::read_to_string(file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    Ok(parsed
        .frontmatter
        .get(field_name)
        .and_then(|value| date_only_timestamp(value)))
}

pub(crate) fn date_in_range(today: NaiveDate, start_date: NaiveDate, end_date: NaiveDate) -> bool {
    today >= start_date && today <= end_date
}

pub(crate) fn relative_path(repo_root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(repo_root).unwrap_or(path).to_path_buf()
}

pub(crate) fn to_forward_slashes(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}
