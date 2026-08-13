use crate::constants::*;
use crate::model::{StoryOverview, TaskSummary};
#[allow(unused_imports)]
use crate::prelude::*;
use crate::util::{parse_assignee_list, relative_path_from, to_forward_slashes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SprintRosterEntry {
    pub(crate) story: StoryOverview,
    pub(crate) link_path: PathBuf,
}

const LEGACY_ROSTER_HEADING: &str = "## Stories (generated — do not edit)";
const ROSTER_SUMMARY_TABLE_HEADER: &str = "| Metric | Stories | Points |";

pub(crate) fn render_sprint_roster(rows: &[SprintRosterEntry]) -> String {
    let mut out = String::new();
    push_line(&mut out, ROSTER_HEADING);
    push_line(&mut out, "");

    render_sprint_roster_summary(&mut out, rows);
    push_line(&mut out, "");

    let mut rows_by_status = BTreeMap::<String, Vec<&SprintRosterEntry>>::new();
    for row in rows {
        rows_by_status
            .entry(row.story.status.clone())
            .or_default()
            .push(row);
    }

    for status in SPRINT_STATUS_DISPLAY_ORDER {
        let mut items = rows_by_status.remove(status).unwrap_or_default();
        items.sort_by(|left, right| left.story.id.cmp(&right.story.id));
        render_sprint_roster_section(&mut out, status, &items);
    }

    for (status, mut items) in rows_by_status {
        items.sort_by(|left, right| left.story.id.cmp(&right.story.id));
        render_sprint_roster_section(&mut out, &status, &items);
    }

    out.trim_end().to_string()
}

pub(crate) fn sprint_story_link_path(
    repo_root: &Path,
    sprint_file_path: &Path,
    story_relative_path: &Path,
) -> PathBuf {
    let sprint_dir = sprint_file_path.parent().unwrap_or(sprint_file_path);
    let story_path = repo_root.join(story_relative_path);
    relative_path_from(sprint_dir, &story_path)
}

pub(crate) fn replace_roster_in_body(body: &str, roster: &str) -> String {
    let trimmed = match roster_replace_start(body) {
        Some(idx) => body[..idx].trim_end().to_string(),
        None => body.trim_end().to_string(),
    };
    format!("{trimmed}\n\n{roster}")
}

fn roster_replace_start(body: &str) -> Option<usize> {
    let anchor = [
        body.find(ROSTER_HEADING),
        body.find(LEGACY_ROSTER_HEADING),
        body.find(ROSTER_SUMMARY_TABLE_HEADER),
    ]
    .into_iter()
    .flatten()
    .min()?;

    let mut search_from = 0;
    let mut containing_conflict_start = None;
    while let Some(rel_start) = body[search_from..].find("<<<<<<<") {
        let start = search_from + rel_start;
        let Some(rel_end) = body[start..].find(">>>>>>>") else {
            break;
        };
        let end = start + rel_end;
        if start <= anchor && anchor <= end {
            containing_conflict_start = Some(start);
            break;
        }
        search_from = end + 7;
    }

    Some(containing_conflict_start.unwrap_or(anchor))
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn render_sprint_roster_section(output: &mut String, status: &str, rows: &[&SprintRosterEntry]) {
    push_line(output, &format!("### {status}"));
    push_line(output, "");

    push_line(output, "| Story | Points | Assignee | Tasks |");
    push_line(output, "|-------|-------:|----------|-------|");

    if rows.is_empty() {
        push_line(output, "| — | — | — | — |");
        push_line(output, "");
        return;
    }

    for row in rows {
        let points = story_points_value(&row.story);
        let assignee = render_assignee_cell(&row.story.assignee);
        let tasks = format_task_summary(row.story.task_summary.as_ref());
        push_line(
            output,
            &format!(
                "| {} | {points} | {assignee} | {tasks} |",
                sprint_story_link_label(row)
            ),
        );
    }

    push_line(output, "");
}

fn render_sprint_roster_summary(output: &mut String, rows: &[SprintRosterEntry]) {
    let mut rows_by_status = BTreeMap::<String, Vec<&SprintRosterEntry>>::new();
    for row in rows {
        rows_by_status
            .entry(row.story.status.clone())
            .or_default()
            .push(row);
    }

    let total_points = rows
        .iter()
        .map(|row| story_points_value(&row.story))
        .sum::<usize>();
    push_line(output, "| Metric | Stories | Points |");
    push_line(output, "|--------|--------:|------:|");
    push_line(
        output,
        &format!("| Total stories | {} | {total_points} |", rows.len()),
    );

    for status in SPRINT_STATUS_DISPLAY_ORDER {
        let items = rows_by_status.remove(status).unwrap_or_default();
        let points = items
            .iter()
            .map(|row| story_points_value(&row.story))
            .sum::<usize>();
        push_line(
            output,
            &format!(
                "| {} | {} | {points} |",
                status_summary_label(status),
                items.len()
            ),
        );
    }
}

fn sprint_story_link_label(row: &SprintRosterEntry) -> String {
    let title = row.story.title.trim();
    let label = if title.is_empty() {
        format!("**{}**", row.story.id)
    } else {
        format!("**{}** {}", row.story.id, title)
    };
    let link_text = escape_markdown_link_text(&label);
    format!("[{link_text}]({})", to_forward_slashes(&row.link_path))
}

fn render_assignee_cell(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" || trimmed.eq_ignore_ascii_case("tbd") {
        return "-".to_string();
    }

    let pattern = &*crate::regexes::ASSIGNEE_CAPTURE;
    let assignees = parse_assignee_list(trimmed);
    let links = assignees
        .iter()
        .filter_map(|assignee| pattern.captures(assignee))
        .filter_map(|captures| {
            let name = captures.name("name")?.as_str().trim();
            let email = captures.name("email")?.as_str().trim();
            if name.is_empty() || email.is_empty() {
                return None;
            }
            Some(format!(
                "[{}](mailto:{})",
                escape_markdown_link_text(name),
                escape_markdown_link_target(email)
            ))
        })
        .collect::<Vec<_>>();

    if links.is_empty() {
        escape_table_cell(trimmed)
    } else {
        links.join(" and ")
    }
}

fn escape_markdown_link_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('|', "\\|")
}

fn escape_markdown_link_target(value: &str) -> String {
    value.replace(' ', "%20")
}

fn escape_table_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', " ")
        .trim()
        .to_string()
}

fn story_points_value(story: &StoryOverview) -> usize {
    story.story_points.trim().parse::<usize>().unwrap_or(0)
}

fn format_task_summary(summary: Option<&TaskSummary>) -> String {
    match summary {
        Some(summary) => format!(
            "✓{} ▶{} ·{} ✗{}",
            summary.done, summary.in_progress, summary.todo, summary.blocked
        ),
        None => "-".to_string(),
    }
}

fn status_summary_label(status: &str) -> &'static str {
    match status {
        "backlog" | "ready" => "Ready",
        "planned" => "Planned",
        "todo" => "Todo",
        "in-progress" => "In progress",
        "ready-for-qa" => "Ready for QA",
        "done" => "Done",
        "blocked" => "Blocked",
        _ => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::{render_assignee_cell, replace_roster_in_body};
    use crate::constants::ROSTER_HEADING;

    #[test]
    fn render_assignee_cell_links_comma_separated_assignees_without_leading_comma() {
        let rendered = render_assignee_cell(
            "Thomas Malt <thomas.malt@vegvesen.no>, Sondre Bjerkerud <sondre.bjerkerud@soprasteria.com>",
        );

        assert_eq!(
            rendered,
            "[Thomas Malt](mailto:thomas.malt@vegvesen.no) and [Sondre Bjerkerud](mailto:sondre.bjerkerud@soprasteria.com)"
        );
    }

    #[test]
    fn replace_roster_in_body_drops_conflict_hunk_containing_roster() {
        let body = "# S004: back-to-school\n\n## Sprint Goal\n\nAlign scope.\n\n<<<<<<< HEAD\n## User Stories selected for sprint\n\n| Metric | Stories | Points |\n|--------|--------:|------:|\n| Total stories | 2 | 13 |\n=======\n## User Stories selected for sprint\n\n| Metric | Stories | Points |\n|--------|--------:|------:|\n| Total stories | 3 | 16 |\n>>>>>>> feature/rebase\n";
        let roster = "## User Stories selected for sprint\n\n| Metric | Stories | Points |\n|--------|--------:|------:|\n| Total stories | 1 | 5 |\n";

        let replaced = replace_roster_in_body(body, roster);
        assert!(replaced.contains("## Sprint Goal"));
        assert!(replaced.contains(ROSTER_HEADING));
        assert!(!replaced.contains("<<<<<<<"));
        assert!(!replaced.contains("======="));
        assert!(!replaced.contains(">>>>>>>"));
        assert!(replaced.contains("| Total stories | 1 | 5 |"));
    }
}
