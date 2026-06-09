#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn print_story_list(theme: &Theme, scope: &str, stories: &[StoryOverview]) {
    print!("{}", render_story_list(theme, scope, stories));
}

pub(crate) fn render_story_list(theme: &Theme, scope: &str, stories: &[StoryOverview]) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "{} {}\n",
        theme.label("Stories:"),
        theme.count(stories.len())
    ));
    output.push_str(&format!("{} {scope}\n", theme.label("Scope:")));
    for story in stories {
        let sprint = story.sprint.as_deref().unwrap_or("~");
        output.push_str(&format!(
            "- {} [{}] sprint={} assignee={} {} {}\n",
            theme.id(&story.id),
            theme.status(&story.status),
            sprint,
            story.assignee,
            theme.story_points(format_story_points(&story.story_points)),
            story.title
        ));
    }
    output
}

pub(crate) fn print_story_details(theme: &Theme, layout: OutputLayout, details: &StoryDetails) {
    print!("{}", render_story_details(theme, layout, details));
}

pub(crate) fn render_story_details(
    theme: &Theme,
    layout: OutputLayout,
    details: &StoryDetails,
) -> String {
    let mut output = String::new();
    push_story_detail_header(&mut output, theme, layout, details);
    push_story_metadata_table(&mut output, theme, layout, details);
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Story Statement",
        details.story_statement.as_deref(),
    );
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Acceptance Criteria",
        details.acceptance_criteria.as_deref(),
    );
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Definition Of Done",
        details.definition_of_done.as_deref(),
    );
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Notes And Open Questions",
        details.notes_and_open_questions.as_deref(),
    );
    push_story_tasks_section(&mut output, theme, layout, details);
    output
}

pub(crate) fn push_story_detail_header(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &StoryDetails,
) {
    let title = format!("{} · {}", details.story.id, details.story.title);
    let status = format!(
        "{} {}",
        status_icon(&details.story.status),
        details.story.status
    );
    let suffix_width = display_width(&status) + 2;
    let title_width = layout.width.saturating_sub(suffix_width).max(1);
    let title_line = wrap_text(&title, title_width)
        .into_iter()
        .next()
        .unwrap_or(title);
    let padding = layout
        .width
        .saturating_sub(display_width(&title_line) + suffix_width);

    push_line(
        output,
        &format!(
            "{}{}  {}",
            highlight_story_id(theme, &title_line),
            " ".repeat(padding),
            theme.status_text(&details.story.status, status)
        ),
    );
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

pub(crate) fn highlight_story_id(theme: &Theme, line: &str) -> String {
    line.split_once(" · ")
        .map(|(id, title)| format!("{} · {}", theme.id(id), theme.heading(title)))
        .unwrap_or_else(|| theme.heading(line))
}

pub(crate) fn push_story_metadata_table(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &StoryDetails,
) {
    push_line(output, "");
    push_line(output, &theme.heading("Overview"));

    let columns = two_column_table_columns(layout.width, 13, "Field", "Value");
    let mut rows = vec![
        metadata_row(
            theme,
            "Status",
            theme.status_text(
                &details.story.status,
                format!(
                    "{} {}",
                    status_icon(&details.story.status),
                    details.story.status
                ),
            ),
            true,
        ),
        metadata_row(
            theme,
            "Sprint",
            details.story.sprint.as_deref().unwrap_or("~").to_string(),
            false,
        ),
        metadata_row(theme, "Assignee", details.story.assignee.clone(), false),
        metadata_row(
            theme,
            "Points",
            theme.story_points(format_story_points(&details.story.story_points)),
            true,
        ),
    ];

    let task_summary = details
        .story
        .task_summary
        .as_ref()
        .map(|summary| format_colored_task_summary(theme, Some(summary)))
        .unwrap_or_else(|| "-".to_string());
    rows.push(metadata_row(theme, "Tasks", task_summary, true));
    if let Some(phase) = story_phase_label(&details.story_file_path) {
        rows.push(metadata_row(theme, "Phase", phase, false));
    }
    if let Some(epic) = story_epic_label(details.epic_id.as_deref(), details.epic_title.as_deref())
    {
        rows.push(metadata_row(theme, "Epic", epic, false));
    }
    rows.push(metadata_row(
        theme,
        "File",
        simplify_story_path(&details.story_file_path),
        false,
    ));
    rows.push(metadata_row(
        theme,
        "Work started",
        details
            .work_started
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("-")
            .to_string(),
        false,
    ));
    rows.push(metadata_row(
        theme,
        "Work done",
        details
            .work_done
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("-")
            .to_string(),
        false,
    ));

    push_wrapped_table(output, theme, &columns, &rows);
}

pub(crate) fn simplify_story_path(path: &Path) -> String {
    // Display the path starting from the phase directory when present so the
    // output is independent of the configured backlog root location.
    match phase_component_index(path) {
        Some(index) => path
            .iter()
            .skip(index)
            .collect::<PathBuf>()
            .display()
            .to_string(),
        None => path.display().to_string(),
    }
}

pub(crate) fn story_phase_label(path: &Path) -> Option<String> {
    let index = phase_component_index(path)?;
    let phase_dir = path.iter().nth(index)?.to_string_lossy();
    phase_dir
        .strip_prefix("phase-")
        .and_then(|rest| rest.split_once('-'))
        .map(|(number, slug)| format!("{} {}", number, headline_from_slug(slug)))
}

fn phase_component_index(path: &Path) -> Option<usize> {
    path.iter()
        .position(|component| component.to_string_lossy().starts_with("phase-"))
}

pub(crate) fn story_epic_label(epic_id: Option<&str>, epic_title: Option<&str>) -> Option<String> {
    let epic_id = epic_id?.trim();
    if epic_id.is_empty() {
        None
    } else {
        let epic_title = epic_title.unwrap_or("").trim();
        if epic_title.is_empty() {
            Some(epic_id.to_string())
        } else {
            Some(format!("{}  {}", epic_id, epic_title))
        }
    }
}

pub(crate) fn headline_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn metadata_row(
    theme: &Theme,
    label: &str,
    value: String,
    precolored: bool,
) -> Vec<TableCell> {
    vec![
        TableCell::preformatted(theme.label(label), CellStyle::Precolored),
        if precolored {
            TableCell::preformatted(value, CellStyle::Precolored)
        } else {
            TableCell::new(value)
        },
    ]
}

pub(crate) fn two_column_table_columns(
    width: usize,
    first_width: usize,
    first_title: &str,
    second_title: &str,
) -> Vec<DynamicTableColumn> {
    let available = row_content_width(width, 2);
    let first_width = first_width.min(available.saturating_sub(1)).max(1);
    vec![
        DynamicTableColumn {
            title: first_title.to_string(),
            width: first_width,
        },
        DynamicTableColumn {
            title: second_title.to_string(),
            width: available.saturating_sub(first_width).max(1),
        },
    ]
}

pub(crate) fn push_story_markdown_section(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    title: &str,
    content: Option<&str>,
) {
    let Some(content) = content.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    push_line(output, "");
    push_line(output, &theme.heading(title));
    push_line(output, &theme.paint(Style::Muted, "─".repeat(title.len())));
    push_terminal_markdown(output, theme, layout.width, content);
}

pub(crate) fn push_story_tasks_section(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &StoryDetails,
) {
    push_line(output, "");
    push_line(output, &theme.heading("Tasks"));
    push_line(output, &theme.paint(Style::Muted, "─────"));
    if details.tasks.is_empty() {
        push_line(output, "  - none");
        return;
    }

    let columns = task_table_columns(layout.width, &details.tasks);
    let rows = details
        .tasks
        .iter()
        .map(|task| {
            vec![
                TableCell::styled(&task.id, CellStyle::Id),
                TableCell::preformatted(
                    theme.status_text(
                        &task.normalized_status,
                        format!(
                            "{} {}",
                            status_icon(&task.normalized_status),
                            task.normalized_status
                        ),
                    ),
                    CellStyle::Precolored,
                ),
                TableCell::new(if task.tags.is_empty() {
                    "-".to_string()
                } else {
                    task.tags.join(", ")
                }),
                TableCell::new(if task.description.trim().is_empty() {
                    task.title.clone()
                } else {
                    format!("{} - {}", task.title, task.description.trim())
                }),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_table(output, theme, &columns, &rows);
}

pub(crate) fn render_task_list(
    theme: &Theme,
    layout: OutputLayout,
    story_id: &str,
    task_file_path: Option<&Path>,
    tasks: &[kanban_core::Task],
) -> String {
    let mut output = String::new();
    push_line(
        &mut output,
        &format!("{} {}", theme.heading("Tasks for"), theme.id(story_id)),
    );
    push_line(
        &mut output,
        &theme.paint(Style::Muted, "─".repeat(layout.width)),
    );
    push_line(&mut output, "");
    push_line(
        &mut output,
        &format!(
            "{} {}",
            theme.label("Task file:"),
            task_file_path
                .map(|path| theme.path(path.display()))
                .unwrap_or_else(|| "-".to_string())
        ),
    );
    if tasks.is_empty() {
        push_line(&mut output, "");
        push_line(&mut output, "  - none");
        return output;
    }

    push_line(&mut output, "");
    let columns = task_table_columns(layout.width, tasks);
    let rows = tasks
        .iter()
        .map(|task| {
            vec![
                TableCell::styled(&task.id, CellStyle::Id),
                TableCell::preformatted(
                    theme.status_text(
                        &task.normalized_status,
                        format!(
                            "{} {}",
                            status_icon(&task.normalized_status),
                            task.normalized_status
                        ),
                    ),
                    CellStyle::Precolored,
                ),
                TableCell::new(if task.tags.is_empty() {
                    "-".to_string()
                } else {
                    task.tags.join(", ")
                }),
                TableCell::new(if task.description.trim().is_empty() {
                    task.title.clone()
                } else {
                    format!("{} - {}", task.title, task.description.trim())
                }),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_table(&mut output, theme, &columns, &rows);
    output
}

pub(crate) fn task_table_columns(
    width: usize,
    tasks: &[kanban_core::Task],
) -> Vec<DynamicTableColumn> {
    let available = row_content_width(width, 4);
    let task_width = tasks
        .iter()
        .map(|task| display_width(&task.id))
        .max()
        .unwrap_or(4)
        .clamp(4, 20);
    let status_width = tasks
        .iter()
        .map(|task| {
            display_width(&format!(
                "{} {}",
                status_icon(&task.normalized_status),
                task.normalized_status
            ))
        })
        .max()
        .unwrap_or(6)
        .clamp(6, 16);
    let tags_width = tasks
        .iter()
        .map(|task| display_width(&task.tags.join(", ")))
        .max()
        .unwrap_or(4)
        .clamp(4, 18);
    let description_width = available
        .saturating_sub(task_width + status_width + tags_width)
        .max(20);

    vec![
        DynamicTableColumn {
            title: "Task".to_string(),
            width: task_width,
        },
        DynamicTableColumn {
            title: "Status".to_string(),
            width: status_width,
        },
        DynamicTableColumn {
            title: "Tags".to_string(),
            width: tags_width,
        },
        DynamicTableColumn {
            title: "Description".to_string(),
            width: description_width,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_story_list_renders_scope_and_story_rows() {
        let theme = Theme::plain();
        let stories = vec![StoryOverview {
            id: "US-F1-010".to_string(),
            title: "CI pipeline with build and unit tests".to_string(),
            status: "in-progress".to_string(),
            epic_id: Some("EP-F1-01".to_string()),
            epic_title: Some("Platform".to_string()),
            assignee: "Ada Lovelace <ada@example.test>".to_string(),
            story_points: "3".to_string(),
            sprint: Some("S000.getting-started".to_string()),
            relative_path: PathBuf::from(
                "delivery/backlog/phase-1-scaffolding/01.some-epic/US-F1-010.md",
            ),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: None,
        }];

        let output = render_story_list(&theme, "active sprint (S000.getting-started)", &stories);

        assert!(output.contains("Stories: 1"));
        assert!(output.contains("Scope: active sprint (S000.getting-started)"));
        assert!(output.contains("US-F1-010 [in-progress] sprint=S000.getting-started"));
        assert!(output.contains("◈3"));
    }

    #[test]
    fn story_details_render_terminal_formatted_markdown() {
        let theme = Theme::plain();
        let details = StoryDetails {
            story: StoryOverview {
                id: "US-F1-010".to_string(),
                title: "CI pipeline with build and unit tests".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("Plattforminfrastruktur".to_string()),
                assignee: "Ada Lovelace <ada@example.test>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S000.getting-started".to_string()),
                relative_path: PathBuf::from(
                    "delivery/backlog/phase-1-scaffolding/01.some-epic/US-F1-010.md",
                ),
                task_summary: Some(TaskSummary {
                    todo: 1,
                    in_progress: 1,
                    blocked: 0,
                    done: 2,
                }),
                task_count: 4,
                work_started: None,
                work_done: None,
            },
            story_file_path: PathBuf::from(
                "delivery/backlog/phase-1-scaffolding/01.plattforminfrastruktur/US-F1-010.md",
            ),
            task_file_path: None,
            epic_id: Some("EP-F1-01".to_string()),
            epic_title: Some("Plattforminfrastruktur".to_string()),
            work_started: Some("2026-05-21T00:00:00+0200".to_string()),
            work_done: None,
            story_statement: Some(
                "As a developer\n\n- I need **formatted** story output".to_string(),
            ),
            acceptance_criteria: Some(
                "Scenario: Show a story\nGiven a story exists\nWhen I run the command\nThen the story is formatted".to_string(),
            ),
            definition_of_done: Some("- [ ] Run `cargo test`".to_string()),
            notes_and_open_questions: Some(
                "| Risk | Mitigation |\n| --- | --- |\n| Raw markdown | Render terminal tables |"
                    .to_string(),
            ),
            tasks: vec![kanban_core::Task {
                id: "TASK-US-F1-010-001".to_string(),
                title: "Build story renderer".to_string(),
                status: "In Progress".to_string(),
                normalized_status: "in-progress".to_string(),
                tags: vec!["cli".to_string()],
                description: "Wire command output".to_string(),
            }],
        };

        let output = render_story_details(&theme, OutputLayout { width: 100 }, &details);

        assert!(output.contains("US-F1-010 · CI pipeline with build and unit tests"));
        assert!(output.contains("Overview"));
        assert!(output.contains("Field"));
        assert!(output.contains("Value"));
        assert!(output.contains("Scenario: Show a story"));
        assert!(output.contains("Given a story exists"));
        assert!(output.contains("☐ Run cargo test"));
        assert!(output.contains("Risk"));
        assert!(output.contains("Mitigation"));
        assert!(output.contains("1 Scaffolding"));
        assert!(output.contains("EP-F1-01 Plattforminfrastruktur"));
        assert!(
            output
                .replace('\\', "/")
                .contains("phase-1-scaffolding/01.plattforminfrastruktur/US-F1-010.md")
        );
        assert!(output.contains("2026-05-21T00:00:00+0200"));
        assert!(output.contains("TASK-US-F1-010-001"));
        assert!(output.contains("→ in-progress"));
        assert!(output.contains("Build story renderer - Wire command output"));
        assert!(!output.contains("Story:"));
        assert!(!output.contains("Task file"));
        assert!(!output.contains("delivery/backlog/"));
        assert!(!output.contains("| Risk | Mitigation |"));
        assert!(!output.contains("- [ ] Run `cargo test`"));
    }
}
