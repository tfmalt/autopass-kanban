#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn print_epic_details(theme: &Theme, layout: OutputLayout, details: &EpicDetails) {
    print!("{}", render_epic_details(theme, layout, details));
}

pub(crate) fn render_epic_details(
    theme: &Theme,
    layout: OutputLayout,
    details: &EpicDetails,
) -> String {
    let mut output = String::new();
    let mut has_content_section = false;

    push_epic_header_band(&mut output, theme, layout, details);
    push_epic_metadata_table(&mut output, theme, layout, details);

    if !details.warnings.is_empty() {
        for warning in &details.warnings {
            push_sprint_section_divider_before_next(
                &mut output,
                theme,
                layout.width,
                &mut has_content_section,
            );
            push_wrapped_hanging_line_inset(&mut output, "", warning, layout.width, |v| {
                theme.warning(v)
            });
        }
    }

    for (title, content) in [
        ("Business Context", details.business_context.as_deref()),
        ("Business Value", details.business_value.as_deref()),
        ("Scope", details.scope.as_deref()),
        (
            "Acceptance Criteria",
            details.acceptance_criteria.as_deref(),
        ),
        (
            "Non-Functional Requirements",
            details.non_functional_requirements.as_deref(),
        ),
        ("Dependencies", details.dependencies.as_deref()),
        ("Definition Of Done", details.definition_of_done.as_deref()),
        (
            "Notes And Open Questions",
            details.notes_and_open_questions.as_deref(),
        ),
    ] {
        let Some(content) = content.map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        push_inset_line(&mut output, &theme.heading(title));
        push_terminal_markdown_indented(&mut output, theme, layout.width, content, 0);
    }

    let full_body = details.body.trim();
    if !full_body.is_empty() {
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        push_inset_line(&mut output, &theme.heading("Epic Body"));
        push_terminal_markdown_indented(&mut output, theme, layout.width, full_body, 0);
    }

    for status in ["todo", "in-progress", "ready-for-qa", "done", "blocked"] {
        let stories = details
            .stories_by_status
            .get(status)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        let icon_label = theme.status_text(status, format!("{} {status}", status_icon(status)));
        let status_points = sum_story_points(stories.iter());
        let points_label = theme.story_points(format_story_points(status_points));
        let story_count = format_story_count(stories.len());
        if stories.is_empty() {
            push_inset_line(
                &mut output,
                &format!(
                    "{icon_label}   {}   {points_label}   · none",
                    theme.count(story_count)
                ),
            );
        } else {
            push_inset_line(
                &mut output,
                &format!(
                    "{icon_label}   {}   {points_label}",
                    theme.count(story_count)
                ),
            );
            let points_width = story_points_column_width(stories.iter());
            push_story_table(
                &mut output,
                theme,
                sprint_story_table_width(layout.width),
                stories,
                points_width,
            );
        }
    }

    output
}

fn push_epic_header_band(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &EpicDetails,
) {
    let title_text = format!("{} · {}", details.epic.id, details.epic.title);
    let status_text = format!(
        "{} {}",
        status_icon(&details.epic.status),
        details.epic.status
    );
    let prefix_text = format!("─── {title_text} ");
    let suffix_text = format!(" {} ───", status_text);
    let fill = layout
        .width
        .saturating_sub(display_width(&prefix_text) + display_width(&suffix_text));
    push_line(
        output,
        &format!(
            "{}{}{}  {} {}",
            theme.paint(Style::Muted, "─── "),
            theme.paint(Style::Cyan, title_text),
            theme.paint(Style::Muted, format!(" {}", "─".repeat(fill))),
            theme.status_text(&details.epic.status, status_text),
            theme.paint(Style::Muted, "───")
        ),
    );

    push_line(output, "");

    let total_points = sum_story_points(details.child_stories.iter());
    let done_points = details
        .stories_by_status
        .get("done")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);
    let in_progress_points = details
        .stories_by_status
        .get("in-progress")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);
    let bar = render_progress_bar(
        theme,
        done_points,
        in_progress_points,
        total_points,
        layout.width,
    );
    let pct = done_points
        .checked_mul(100)
        .and_then(|value| value.checked_div(total_points.max(1)))
        .unwrap_or(0);
    push_line(
        output,
        &format!(
            "  {}  {}  {}",
            bar,
            theme.story_points(format!(
                "{} / {}",
                format_story_points(done_points),
                total_points
            )),
            theme.paint(Style::Muted, format!("{pct}%")),
        ),
    );

    let done = details
        .stories_by_status
        .get("done")
        .map(|v| v.len())
        .unwrap_or(0);
    let in_progress = details
        .stories_by_status
        .get("in-progress")
        .map(|v| v.len())
        .unwrap_or(0);
    let qa = details
        .stories_by_status
        .get("ready-for-qa")
        .map(|v| v.len())
        .unwrap_or(0);
    let todo = details
        .stories_by_status
        .get("todo")
        .map(|v| v.len())
        .unwrap_or(0);
    let blocked = details
        .stories_by_status
        .get("blocked")
        .map(|v| v.len())
        .unwrap_or(0);
    let dot = theme.paint(Style::Muted, "·");
    let segments: Vec<String> = [
        (done, "done", Style::Green),
        (in_progress, "in progress", Style::Blue),
        (qa, "in qa", Style::Purple),
        (todo, "todo", Style::Muted),
        (blocked, "blocked", Style::Red),
    ]
    .into_iter()
    .map(|(count, label, style)| {
        let s = if count == 0 { Style::Muted } else { style };
        theme.paint(s, format!("{count} {label}"))
    })
    .collect();
    push_line(
        output,
        &format!("  {}", segments.join(&format!("  {dot}  "))),
    );

    push_line(output, "");
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

fn push_epic_metadata_table(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &EpicDetails,
) {
    push_line(output, "");
    push_line(output, &theme.heading("Overview"));

    let columns = two_column_table_columns(layout.width, 13, "Field", "Value");
    let mut rows = vec![
        metadata_row(
            theme,
            "Status",
            theme.status_text(
                &details.epic.status,
                format!(
                    "{} {}",
                    status_icon(&details.epic.status),
                    details.epic.status
                ),
            ),
            true,
        ),
        metadata_row(
            theme,
            "Stories",
            theme.count(format_story_count(details.child_stories.len())),
            true,
        ),
        metadata_row(
            theme,
            "Points",
            theme.story_points(format_story_points(sum_story_points(
                details.child_stories.iter(),
            ))),
            true,
        ),
    ];

    if let Some(phase) = details.epic.phase.as_deref() {
        rows.push(metadata_row(theme, "Phase", phase.to_string(), false));
    }
    if let Some(owner) = details.epic.owner.as_deref() {
        rows.push(metadata_row(theme, "Owner", owner.to_string(), false));
    }
    if let Some(milestone) = details.epic.milestone.as_deref() {
        rows.push(metadata_row(
            theme,
            "Milestone",
            milestone.to_string(),
            false,
        ));
    }
    rows.push(metadata_row(
        theme,
        "File",
        simplify_story_path(&details.epic.relative_path),
        false,
    ));

    push_wrapped_table(output, theme, &columns, &rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epic_details_render_progress_and_sections() {
        let theme = Theme::plain();
        let details = EpicDetails {
            epic: EpicOverview {
                id: "EP-F1-06".to_string(),
                title: "Git-driven kanban and backlog tooling".to_string(),
                status: "draft".to_string(),
                phase: Some("1".to_string()),
                owner: Some("Solution Architect / Product Owner".to_string()),
                milestone: Some("MP1".to_string()),
                relative_path: PathBuf::from(
                    "delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-06-git-driven-kanban-and-backlog-tooling.md",
                ),
            },
            story_ids: vec!["US-F1-052".to_string(), "US-F1-053".to_string()],
            stories_by_status: BTreeMap::from([
                (
                    "done".to_string(),
                    vec![StoryOverview {
                        id: "US-F1-052".to_string(),
                        title: "Add read-only CLI for sprint and backlog inspection".to_string(),
                        status: "done".to_string(),
                        epic_id: Some("EP-F1-06".to_string()),
                        epic_title: Some("Git-driven kanban and backlog tooling".to_string()),
                        assignee: "TBD".to_string(),
                        story_points: "5".to_string(),
                        sprint: Some("S000.getting-started".to_string()),
                        relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-052.md"),
                        task_summary: None,
                        task_count: 0,
                        work_started: None,
                        work_done: None,
                        planned_start: None,
                        planned_end: None,
                    }],
                ),
                (
                    "in-progress".to_string(),
                    vec![StoryOverview {
                        id: "US-F1-053".to_string(),
                        title: "Add CLI support for status moves and sprint rollover".to_string(),
                        status: "in-progress".to_string(),
                        epic_id: Some("EP-F1-06".to_string()),
                        epic_title: Some("Git-driven kanban and backlog tooling".to_string()),
                        assignee: "TBD".to_string(),
                        story_points: "8".to_string(),
                        sprint: Some("S000.getting-started".to_string()),
                        relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-053.md"),
                        task_summary: None,
                        task_count: 0,
                        work_started: None,
                        work_done: None,
                        planned_start: None,
                        planned_end: None,
                    }],
                ),
            ]),
            child_stories: vec![],
            warnings: vec![
                "Epic status is `draft` but child stories are `in-progress`. Update the epic status to reflect active work.".to_string(),
            ],
            body: "# Epic: Git-driven kanban and backlog tooling\n\n## Business Context\n\nMarkdown-first workflow.\n\n## Scope\n\n- Keep tooling simple".to_string(),
            business_context: Some("Markdown-first workflow.".to_string()),
            business_value: Some("Faster inspection.".to_string()),
            scope: None,
            acceptance_criteria: Some("- Show epics".to_string()),
            non_functional_requirements: None,
            dependencies: None,
            definition_of_done: None,
            notes_and_open_questions: None,
        };

        let output = render_epic_details(&theme, OutputLayout { width: 100 }, &details);

        assert!(output.contains("EP-F1-06 · Git-driven kanban and backlog tooling"));
        assert!(output.contains("Overview"));
        assert!(output.contains("Business Context"));
        assert!(output.contains("Acceptance Criteria"));
        assert!(output.contains("Epic Body"));
        assert!(
            output.contains("# Epic: Git-driven kanban and backlog tooling")
                || output.contains("Epic: Git-driven kanban and backlog tooling")
        );
        assert!(output.contains("US-F1-052"));
        assert!(output.contains("US-F1-053"));
        assert!(output.contains("child stories are `in-progress`"));
    }
}
