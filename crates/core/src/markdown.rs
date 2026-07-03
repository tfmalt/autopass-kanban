use crate::constants::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::atomic_write;
use crate::util::*;

pub fn parse_frontmatter(markdown: &str) -> ParsedFrontmatter {
    let normalized = markdown.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") {
        return ParsedFrontmatter {
            frontmatter: BTreeMap::new(),
            frontmatter_keys: BTreeSet::new(),
            body: normalized,
        };
    }

    let lines: Vec<&str> = normalized.split('\n').collect();
    let closing_index = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index));

    let Some(closing_index) = closing_index else {
        return ParsedFrontmatter {
            frontmatter: BTreeMap::new(),
            frontmatter_keys: BTreeSet::new(),
            body: normalized,
        };
    };

    let mut frontmatter = BTreeMap::new();
    let mut frontmatter_keys = BTreeSet::new();

    for line in &lines[1..closing_index] {
        if line.trim().is_empty() {
            continue;
        }

        let Some((key, raw_value)) = line.split_once(':') else {
            continue;
        };

        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }

        frontmatter_keys.insert(key.to_string());
        frontmatter.insert(key.to_string(), parse_scalar(raw_value));
    }

    ParsedFrontmatter {
        frontmatter,
        frontmatter_keys,
        body: lines[(closing_index + 1)..].join("\n"),
    }
}

pub fn parse_task_markdown(markdown: &str) -> Vec<Task> {
    let normalized = markdown.replace("\r\n", "\n");
    let heading_pattern = Regex::new(TASK_HEADING_PATTERN).expect("valid task heading regex");
    let matches: Vec<_> = heading_pattern
        .captures_iter(&normalized)
        .filter_map(|captures| {
            let full = captures.get(0)?;
            let id = captures.get(1)?.as_str().to_string();
            let title = captures.get(2)?.as_str().trim().to_string();
            Some((full.start(), full.end(), id, title))
        })
        .collect();

    let mut tasks = Vec::new();
    for (index, (_, block_start, id, title)) in matches.iter().enumerate() {
        let block_end = matches
            .get(index + 1)
            .map(|next| next.0)
            .unwrap_or(normalized.len());
        let block = &normalized[*block_start..block_end];
        let status = capture_line_value(block, "Status").unwrap_or_default();
        let tags_value = capture_line_value(block, "Tags").unwrap_or_default();
        let description = capture_description(block);

        tasks.push(Task {
            id: id.clone(),
            title: title.clone(),
            normalized_status: normalize_task_status(&status),
            status,
            tags: tags_value
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            description,
        });
    }

    tasks
}

pub(crate) fn task_file_uses_legacy_separators(markdown: &str) -> bool {
    markdown.lines().any(|line| line.trim() == "---")
}

pub(crate) fn render_task_file(story_id: &str, sprint_name: &str, tasks: &[Task]) -> String {
    let mut output =
        format!("# Tasks for {story_id}\n\nParent User Story: {story_id}\nSprint: {sprint_name}");

    for task in tasks {
        output.push_str("\n\n");
        output.push_str(&render_task_block(
            &task.id,
            &task.title,
            &task.normalized_status,
            &task.tags,
            &task.description,
        ));
    }

    output.push('\n');
    output
}

pub fn create_task_summary(tasks: &[Task]) -> TaskSummary {
    // `ready-for-qa` and any other unrecognized status are intentionally
    // dropped here (not folded into `todo`), matching the pre-existing
    // behavior of `TaskSummary`, which only tracks the four canonical
    // task-board buckets.
    let counts = TaskStatusCounts::count(tasks.iter().map(|task| task.normalized_status.as_str()));
    TaskSummary {
        todo: counts.todo,
        in_progress: counts.in_progress,
        blocked: counts.blocked,
        done: counts.done,
    }
}

pub(crate) fn frontmatter_region(markdown: &str) -> Result<&str> {
    let mut offset = 0;
    for (index, line) in markdown.split_inclusive('\n').enumerate() {
        offset += line.len();
        let marker = line.trim_end_matches(['\r', '\n']);
        if index == 0 && marker != "---" {
            bail!("Markdown file is missing YAML frontmatter.");
        }
        if index > 0 && marker == "---" {
            return Ok(&markdown[..offset]);
        }
    }
    bail!("Markdown file has an unclosed frontmatter block.")
}

pub fn update_story_frontmatter_markdown(
    markdown: &str,
    updates: &[(&str, Option<String>)],
) -> Result<String> {
    let normalized = markdown.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    if !normalized.starts_with("---\n") {
        bail!("Story file is missing YAML frontmatter.");
    }
    let closing_index = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index))
        .ok_or_else(|| anyhow!("Story file has an unclosed frontmatter block."))?;

    let mut output = Vec::new();
    output.push("---".to_string());
    for line in &lines[1..closing_index] {
        if line.trim().is_empty() {
            output.push(String::new());
            continue;
        }
        if let Some((key, _)) = line.split_once(':') {
            let key = key.trim();
            if let Some((_, value)) = updates.iter().find(|(candidate, _)| *candidate == key) {
                output.push(format!("{key}: {}", value.clone().unwrap_or_default()));
                continue;
            }
        }
        output.push((*line).to_string());
    }
    output.push("---".to_string());
    output.extend(
        lines[(closing_index + 1)..]
            .iter()
            .map(|line| (*line).to_string()),
    );
    Ok(output.join("\n"))
}

pub fn upsert_frontmatter_markdown(
    markdown: &str,
    updates: &[(&str, Option<String>)],
) -> Result<String> {
    let normalized = markdown.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    if !normalized.starts_with("---\n") {
        bail!("Story file is missing YAML frontmatter.");
    }
    let closing_index = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (*line == "---").then_some(index))
        .ok_or_else(|| anyhow!("Story file has an unclosed frontmatter block."))?;

    let parsed = parse_frontmatter(&normalized);
    let mut output = Vec::new();
    let mut applied = BTreeSet::new();

    output.push("---".to_string());
    for line in &lines[1..closing_index] {
        if line.trim().is_empty() {
            output.push(String::new());
            continue;
        }
        if let Some((key, _)) = line.split_once(':') {
            let key = key.trim();
            if let Some((_, value)) = updates.iter().find(|(candidate, _)| *candidate == key) {
                applied.insert(key.to_string());
                output.push(format!("{key}: {}", value.clone().unwrap_or_default()));
                continue;
            }
        }
        output.push((*line).to_string());
    }

    for (key, value) in updates {
        if parsed.frontmatter_keys.contains(*key) || applied.contains(*key) {
            continue;
        }
        output.push(format!("{key}: {}", value.clone().unwrap_or_default()));
    }

    output.push("---".to_string());
    output.extend(
        lines[(closing_index + 1)..]
            .iter()
            .map(|line| (*line).to_string()),
    );
    Ok(output.join("\n"))
}

pub(crate) fn upsert_story_frontmatter_file(
    file_path: &Path,
    updates: &[(&str, Option<String>)],
) -> Result<()> {
    let markdown = fs::read_to_string(file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let updated = upsert_frontmatter_markdown(&markdown, updates)?;
    atomic_write(file_path, &updated)
        .with_context(|| format!("write story file {}", file_path.display()))?;
    Ok(())
}

pub(crate) fn render_empty_task_file(story_id: &str, sprint_name: &str) -> String {
    render_task_file(story_id, sprint_name, &[])
}

pub(crate) fn next_task_id(story: &Story, task_file: &TaskFile) -> String {
    let story_id = story.fields.id.clone();
    let next_number = task_file
        .tasks
        .iter()
        .filter_map(|task| task.id.rsplit('-').next()?.parse::<u32>().ok())
        .max()
        .map(|value| value + 1)
        .unwrap_or(1);
    format!("TASK-{story_id}-{next_number:03}")
}

pub(crate) fn append_task_markdown(
    markdown: &str,
    task_id: &str,
    title: &str,
    status: &str,
    tags: &[String],
    description: &str,
) -> String {
    let mut tasks = parse_task_markdown(markdown);
    let story_id = task_id
        .strip_prefix("TASK-")
        .and_then(|value| value.rsplit_once('-').map(|(prefix, _)| prefix))
        .unwrap_or_default();
    let sprint_name = capture_line_value(markdown, "Sprint").unwrap_or_else(|| "~".to_string());
    tasks.push(Task {
        id: task_id.to_string(),
        title: title.trim().to_string(),
        status: display_task_status(status).to_string(),
        normalized_status: normalize_task_status(status),
        tags: tags.to_vec(),
        description: description.trim().to_string(),
    });
    render_task_file(story_id, &sprint_name, &tasks)
}

pub(crate) fn rewrite_task_markdown(
    markdown: &str,
    task_id: &str,
    status: Option<&str>,
    title: Option<&str>,
    tags: Option<&[String]>,
    description: Option<&str>,
) -> Result<String> {
    let normalized = markdown.replace("\r\n", "\n");
    let heading_pattern = Regex::new(TASK_HEADING_PATTERN).expect("valid task heading regex");
    let matches: Vec<_> = heading_pattern
        .captures_iter(&normalized)
        .filter_map(|captures| {
            let full = captures.get(0)?;
            let id = captures.get(1)?.as_str().to_string();
            let title = captures.get(2)?.as_str().trim().to_string();
            Some((full.start(), full.end(), id, title))
        })
        .collect();

    let normalized_task_id = task_id.trim().to_ascii_uppercase();
    let mut found = false;
    let file_story_id = capture_line_value(&normalized, "Parent User Story").unwrap_or_default();
    let sprint_name = capture_line_value(&normalized, "Sprint").unwrap_or_else(|| "~".to_string());
    let mut rewritten_tasks = Vec::new();

    for (index, (_start, block_start, id, existing_title)) in matches.iter().enumerate() {
        let block_end = matches
            .get(index + 1)
            .map(|next| next.0)
            .unwrap_or(normalized.len());
        let block = &normalized[*block_start..block_end];
        let existing_status = capture_line_value(block, "Status").unwrap_or_default();
        let existing_tags = capture_line_value(block, "Tags")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let existing_description = capture_description(block);
        if id.eq_ignore_ascii_case(&normalized_task_id) {
            rewritten_tasks.push(Task {
                id: id.clone(),
                title: title.unwrap_or(existing_title).trim().to_string(),
                status: display_task_status(status.unwrap_or(existing_status.trim())).to_string(),
                normalized_status: normalize_task_status(status.unwrap_or(existing_status.trim())),
                tags: tags.unwrap_or(existing_tags.as_slice()).to_vec(),
                description: description
                    .unwrap_or(existing_description.as_str())
                    .trim()
                    .to_string(),
            });
            found = true;
        } else {
            rewritten_tasks.push(Task {
                id: id.clone(),
                title: existing_title.clone(),
                status: display_task_status(existing_status.trim()).to_string(),
                normalized_status: normalize_task_status(existing_status.trim()),
                tags: existing_tags,
                description: existing_description,
            });
        }
    }

    if found {
        Ok(render_task_file(
            &file_story_id,
            &sprint_name,
            &rewritten_tasks,
        ))
    } else {
        bail!("Task not found: {normalized_task_id}");
    }
}

pub(crate) fn render_task_block(
    task_id: &str,
    title: &str,
    status: &str,
    tags: &[String],
    description: &str,
) -> String {
    format!(
        "## {task_id} - {}\n\nStatus: {}\nTags: {}\n\nDescription:\n{}",
        title.trim(),
        display_task_status(status),
        tags.join(", "),
        description.trim()
    )
}

pub(crate) fn display_task_status(status: &str) -> &'static str {
    match normalize_task_status(status).as_str() {
        "todo" => "todo",
        "in-progress" => "in-progress",
        "blocked" => "blocked",
        "done" => "done",
        _ => "todo",
    }
}

pub(crate) fn replace_markdown_section(markdown: &str, heading: &str, new_content: &str) -> String {
    let normalized = markdown.replace("\r\n", "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let target_heading = format!("## {heading}");
    let Some(start) = lines.iter().position(|line| line.trim() == target_heading) else {
        let mut output = normalized.trim_end().to_string();
        output.push_str("\n\n");
        output.push_str(&target_heading);
        output.push_str("\n\n");
        output.push_str(new_content.trim());
        output.push('\n');
        return output;
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());

    let mut output = Vec::new();
    output.extend(lines[..=start].iter().map(|line| (*line).to_string()));
    output.push(String::new());
    output.extend(new_content.trim().lines().map(|line| line.to_string()));
    output.push(String::new());
    output.extend(lines[end..].iter().map(|line| (*line).to_string()));
    output.join("\n")
}

pub(crate) fn story_title(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let title = line.strip_prefix("# ")?.trim();
        let title = title
            .strip_prefix("User Story: ")
            .or_else(|| title.strip_prefix("Epic: "))
            .unwrap_or(title);
        Some(title.to_string())
    })
}

pub fn title_from_body(body: &str, prefix: &str) -> String {
    body.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(|title| {
            title
                .trim()
                .strip_prefix(&format!("{prefix}: "))
                .unwrap_or(title.trim())
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

pub fn phase_from_id(id: &str, prefix: &str) -> Option<String> {
    let marker = format!("{prefix}-F");
    let start = id.to_ascii_uppercase().find(&marker)? + prefix.len() + 1;
    let rest = &id[start..];
    let end = rest.find('-').unwrap_or(rest.len());
    let phase = &rest[..end];
    (!phase.is_empty()).then(|| phase.to_ascii_uppercase())
}

pub fn frontmatter_option(value: Option<&String>) -> Option<String> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "~" && *value != "null")
        .map(str::to_string)
}

pub fn parse_i64_frontmatter(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok()
}

pub fn parse_non_negative_i64_frontmatter(value: &str) -> Option<i64> {
    parse_i64_frontmatter(value).filter(|value| *value >= 0)
}

pub fn extract_markdown_section(body: &str, heading: &str) -> Option<String> {
    let normalized = body.replace("\r\n", "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let target_heading = format!("## {heading}");
    let start = lines
        .iter()
        .position(|line| line.trim() == target_heading)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());

    let section_lines = lines[(start + 1)..end]
        .iter()
        .copied()
        .skip_while(|line| line.trim().is_empty() || line.trim() == "---")
        .collect::<Vec<_>>();
    let mut section = section_lines.join("\n").trim().to_string();
    while section.ends_with("\n---") || section == "---" {
        section = section.trim_end_matches("---").trim_end().to_string();
    }
    (!section.is_empty()).then_some(section)
}

pub(crate) fn parse_scalar(raw_value: &str) -> String {
    let value = raw_value.trim();
    if value.is_empty() {
        return String::new();
    }
    if value == "~" {
        return "~".to_string();
    }
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return value[1..value.len() - 1].to_string();
    }
    value.to_string()
}

pub(crate) fn normalize_task_status(status: &str) -> String {
    normalize_status_alias(status)
}

pub(crate) fn capture_line_value(block: &str, prefix: &str) -> Option<String> {
    block.lines().find_map(|line| {
        let (left, right) = line.split_once(':')?;
        (left.trim() == prefix).then(|| right.trim().to_string())
    })
}

pub(crate) fn capture_description(block: &str) -> String {
    let marker = "Description:\n";
    let Some(start) = block.find(marker) else {
        return String::new();
    };
    let mut description = block[(start + marker.len())..].trim().to_string();
    if let Some(stripped) = description.strip_suffix("---") {
        description = stripped.trim_end().to_string();
    }
    description
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_region_preserves_crlf_closing_delimiter() {
        let markdown = "---\r\nsprint: S001\r\nheadline: foundation\r\nwip_limit: null\r\n---\r\n\r\n# S001: foundation\r\n";

        let region = frontmatter_region(markdown).unwrap();

        assert_eq!(
            region,
            "---\r\nsprint: S001\r\nheadline: foundation\r\nwip_limit: null\r\n---\r\n"
        );
    }

    #[test]
    fn task_update_preserves_other_task_headings() {
        let markdown = "# Tasks for US-F1-053\n\n---\n\n## TASK-US-F1-053-001 - First task\n\nStatus: To Do\nTags: docs\n\nDescription:\nFirst.\n\n---\n\n## TASK-US-F1-053-002 - Second task\n\nStatus: To Do\nTags: cli\n\nDescription:\nSecond.\n\n---\n\n## TASK-US-F1-053-003 - Third task\n\nStatus: To Do\nTags: tests\n\nDescription:\nThird.\n";

        let updated = rewrite_task_markdown(
            markdown,
            "TASK-US-F1-053-002",
            Some("done"),
            None,
            None,
            None,
        )
        .unwrap();

        assert!(updated.contains("## TASK-US-F1-053-001 - First task"));
        assert!(updated.contains("## TASK-US-F1-053-002 - Second task"));
        assert!(updated.contains("## TASK-US-F1-053-003 - Third task"));
        assert!(updated.contains("Status: done"));
        assert!(updated.contains("Description:\nSecond.\n\n## TASK-US-F1-053-003 - Third task"));
    }

    #[test]
    fn display_task_status_normalizes_legacy_labels_to_canonical_keywords() {
        assert_eq!(display_task_status("To Do"), "todo");
        assert_eq!(display_task_status("In Progress"), "in-progress");
        assert_eq!(display_task_status("Done"), "done");
    }

    // Tests for title_from_body

    #[test]
    fn title_from_body_extracts_title_with_user_story_prefix() {
        let body = "# User Story: Implement login feature\n\nSome description here";
        assert_eq!(
            title_from_body(body, "User Story"),
            "Implement login feature"
        );
    }

    #[test]
    fn title_from_body_extracts_title_with_epic_prefix() {
        let body = "# Epic: Authentication System\n\nDetails.";
        assert_eq!(title_from_body(body, "Epic"), "Authentication System");
    }

    #[test]
    fn title_from_body_returns_full_title_when_prefix_not_present() {
        let body = "# Just a title\n\nBody text.";
        assert_eq!(title_from_body(body, "User Story"), "Just a title");
    }

    #[test]
    fn title_from_body_returns_empty_string_when_no_heading() {
        let body = "No heading here.\nJust prose.";
        assert_eq!(title_from_body(body, "User Story"), "");
    }

    #[test]
    fn title_from_body_trims_extra_whitespace() {
        let body = "#   User Story:   Extra spaces   \n";
        assert_eq!(title_from_body(body, "User Story"), "Extra spaces");
    }

    #[test]
    fn title_from_body_returns_empty_string_for_empty_body() {
        assert_eq!(title_from_body("", "User Story"), "");
    }

    #[test]
    fn title_from_body_uses_first_heading_only() {
        let body = "# User Story: First title\n\n# User Story: Second title\n";
        assert_eq!(title_from_body(body, "User Story"), "First title");
    }

    #[test]
    fn title_from_body_strips_custom_prefix() {
        let body = "# MyPrefix: The actual title\n";
        assert_eq!(title_from_body(body, "MyPrefix"), "The actual title");
    }

    #[test]
    fn title_from_body_preserves_colons_inside_title() {
        let body = "# User Story: Time: 3:00 PM\n";
        assert_eq!(title_from_body(body, "User Story"), "Time: 3:00 PM");
    }

    // Tests for phase_from_id

    #[test]
    fn phase_from_id_extracts_phase_from_story_id() {
        assert_eq!(phase_from_id("US-F1-061", "US"), Some("F1".to_string()));
    }

    #[test]
    fn phase_from_id_extracts_phase_from_epic_id() {
        assert_eq!(phase_from_id("EP-F2-034", "EP"), Some("F2".to_string()));
    }

    #[test]
    fn phase_from_id_extracts_multi_digit_phase() {
        assert_eq!(phase_from_id("US-F12-061", "US"), Some("F12".to_string()));
    }

    #[test]
    fn phase_from_id_returns_none_when_marker_not_found() {
        assert_eq!(phase_from_id("INVALID-061", "US"), None);
    }

    #[test]
    fn phase_from_id_returns_bare_f_when_no_phase_number() {
        assert_eq!(phase_from_id("US-F-061", "US"), Some("F".to_string()));
    }

    #[test]
    fn phase_from_id_returns_none_for_empty_id() {
        assert_eq!(phase_from_id("", "US"), None);
    }

    #[test]
    fn phase_from_id_marker_match_requires_prefix_case() {
        // The id is uppercased before searching, but the prefix marker is
        // used as given — a lowercase prefix can never match.
        assert_eq!(phase_from_id("us-f3-042", "us"), None);
    }

    #[test]
    fn phase_from_id_supports_longer_prefixes() {
        assert_eq!(phase_from_id("ABC-F5-100", "ABC"), Some("F5".to_string()));
    }

    #[test]
    fn phase_from_id_uppercases_phase_output() {
        assert_eq!(phase_from_id("US-f1-061", "US"), Some("F1".to_string()));
    }

    #[test]
    fn phase_from_id_returns_none_for_wrong_prefix() {
        assert_eq!(phase_from_id("US-F1-061", "EP"), None);
    }

    #[test]
    fn phase_from_id_requires_f_marker_after_prefix() {
        assert_eq!(phase_from_id("US-1-061", "US"), None);
    }
}
