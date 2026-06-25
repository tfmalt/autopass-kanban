use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use chrono::NaiveDate;
use kanban_core::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::snapshot::rel_to_root;

fn validate_sprint_name_segment(name: &str) -> Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains('\0') {
        bail!("invalid sprint name");
    }
    let Some((prefix, headline)) = name.split_once('.') else {
        bail!("invalid sprint name");
    };
    if !prefix.starts_with('S')
        || prefix.len() < 2
        || !prefix[1..].chars().all(|ch| ch.is_ascii_digit())
    {
        bail!("invalid sprint name");
    }
    if headline.is_empty()
        || !headline
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        || headline.starts_with('-')
        || headline.ends_with('-')
    {
        bail!("invalid sprint name");
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSprintInputWeb {
    pub(crate) headline: String,
    pub(crate) number: Option<u32>,
    pub(crate) start: Option<String>,
    pub(crate) end: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateSprintInput {
    pub(crate) headline: String,
    pub(crate) goal: String,
    pub(crate) start: String,
    pub(crate) end: String,
    pub(crate) status: String,
    pub(crate) wip_limit: Option<i64>,
}

pub(crate) fn update_sprint_file(
    repo_root: &Path,
    name: &str,
    input: UpdateSprintInput,
) -> Result<Value> {
    validate_sprint_name_segment(name)?;
    let config = load_kanban_config(repo_root)?;
    let old_path = config.sprints_path().join(format!("{name}.md"));
    let content = fs::read_to_string(&old_path)
        .with_context(|| format!("read sprint file {}", old_path.display()))?;
    let parsed = parse_frontmatter(&content);
    let sprint_id = parsed
        .frontmatter
        .get("sprint")
        .cloned()
        .unwrap_or_else(|| name.split('.').next().unwrap_or(name).to_string());
    let headline = slugify(&input.headline);
    if headline.is_empty() {
        bail!("Sprint headline must contain at least one ASCII letter or number.");
    }
    let new_name = format!("{sprint_id}.{headline}");
    validate_sprint_name_segment(&new_name)?;
    let new_path = config.sprints_path().join(format!("{new_name}.md"));
    if new_name != name && new_path.exists() {
        bail!("Sprint file already exists: {new_name}.md");
    }
    let mut updates = BTreeMap::new();
    updates.insert("headline".to_string(), headline.clone());
    updates.insert("start_date".to_string(), input.start);
    updates.insert("end_date".to_string(), input.end);
    updates.insert("status".to_string(), input.status);
    updates.insert(
        "wip_limit".to_string(),
        input
            .wip_limit
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string()),
    );
    let mut updated = replace_frontmatter_fields(&content, &updates)?;
    updated = replace_section_content(&updated, "Sprint Goal", &input.goal);
    updated = replace_sprint_title(&updated, &sprint_id, &headline);
    atomic_write(&old_path, &updated)
        .with_context(|| format!("write sprint file {}", old_path.display()))?;
    if new_name != name {
        update_story_sprint_references(repo_root, name, &new_name)?;
        fs::rename(&old_path, &new_path)
            .with_context(|| format!("rename sprint file to {}", new_path.display()))?;
    }
    Ok(json!({
        "sprintPath": rel_to_root(repo_root, &new_path),
        "name": new_name,
        "headline": headline
    }))
}

pub(crate) fn update_story_sprint_references(
    repo_root: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<()> {
    let repository = read_repository(repo_root)?;
    for story in repository.stories {
        if story.frontmatter.get("sprint").map(String::as_str) == Some(old_name) {
            let updates = [("sprint".to_string(), new_name.to_string())];
            update_story_frontmatter(
                repo_root,
                story
                    .frontmatter
                    .get("id")
                    .map(String::as_str)
                    .unwrap_or_default(),
                &updates,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn parse_date_or(value: Option<&str>, fallback: NaiveDate) -> Result<NaiveDate> {
    match value.filter(|value| !value.trim().is_empty()) {
        Some(value) => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .with_context(|| format!("parse date {value}")),
        None => Ok(fallback),
    }
}

pub(crate) fn replace_frontmatter_fields(
    markdown: &str,
    updates: &BTreeMap<String, String>,
) -> Result<String> {
    let newline = if markdown.starts_with("---\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if !markdown.starts_with("---") {
        bail!("markdown file does not start with frontmatter");
    }
    let close = format!("{newline}---");
    let end = markdown[3..]
        .find(&close)
        .ok_or_else(|| anyhow!("frontmatter is not closed"))?
        + 3;
    let frontmatter = &markdown[..end];
    let rest = &markdown[end..];
    let mut lines = frontmatter.lines().map(str::to_string).collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    for line in &mut lines {
        if let Some((key, _)) = line.split_once(':') {
            let key = key.trim().to_string();
            if let Some(value) = updates.get(&key) {
                *line = format!("{}: {}", key, value);
                seen.insert(key);
            }
        }
    }
    for (key, value) in updates {
        if !seen.contains(key) {
            lines.push(format!("{key}: {value}"));
        }
    }
    Ok(format!("{}{}", lines.join(newline), rest))
}

pub(crate) fn replace_section_content(markdown: &str, heading: &str, value: &str) -> String {
    let marker = format!("## {heading}");
    let Some(index) = markdown.find(&marker) else {
        return markdown.to_string();
    };
    let start = index + marker.len();
    let next = markdown[start..]
        .find("\n## ")
        .map(|offset| start + offset + 1)
        .unwrap_or(markdown.len());
    format!(
        "{}{}\n\n{}\n{}",
        &markdown[..index],
        marker,
        value.trim(),
        &markdown[next..]
    )
}

pub(crate) fn replace_sprint_title(markdown: &str, sprint_id: &str, headline: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            if line.starts_with("# ") {
                format!("# {sprint_id}: {headline}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in value.trim().chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn slugify_headline_keeps_ascii_tokens() {
        assert_eq!(slugify("Foundation Sprint!"), "foundation-sprint");
    }

    #[test]
    fn invalid_sprint_name_segment_is_rejected() {
        for value in ["../settings", "S1/evil", "S1\\evil", "S1..evil", "bad"] {
            let err = validate_sprint_name_segment(value).unwrap_err().to_string();
            assert!(
                err.contains("invalid sprint name"),
                "value={value:?} err={err}"
            );
        }
    }

    #[test]
    fn update_sprint_file_rejects_invalid_route_name_before_fs_join() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();
        let result = update_sprint_file(
            temp_root.path(),
            "../settings",
            UpdateSprintInput {
                headline: "foundation".to_string(),
                goal: String::new(),
                start: "2026-06-01".to_string(),
                end: "2026-06-12".to_string(),
                status: "active".to_string(),
                wip_limit: None,
            },
        );
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid sprint name"));
        assert!(!temp_root.path().join("settings.md").exists());
    }
}
