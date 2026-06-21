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
    let layout = OutputLayout::for_stdout().unwrap_or(OutputLayout { width: 80 });
    print!("{}", render_story_list(theme, layout, scope, stories));
}

fn normalize_story_status(status: &str) -> String {
    match status.trim().to_ascii_lowercase().as_str() {
        "backlog" => "ready".to_string(),
        "to do" => "todo".to_string(),
        "in progress" => "in-progress".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn render_story_list(
    theme: &Theme,
    layout: OutputLayout,
    scope: &str,
    stories: &[StoryOverview],
) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "{} {}\n",
        theme.label("Stories:"),
        theme.count(stories.len())
    ));
    output.push_str(&format!("{} {scope}\n", theme.label("Scope:")));

    let story_table_width = sprint_story_table_width(layout.width);

    let status_order = [
        "draft",
        "ready",
        "todo",
        "in-progress",
        "ready-for-qa",
        "done",
        "blocked",
        "dropped",
    ];
    let mut by_status: BTreeMap<&str, Vec<&StoryOverview>> = BTreeMap::new();
    for story in stories {
        let normalized = normalize_story_status(&story.status);
        let key = status_order
            .iter()
            .find(|&&s| s == normalized)
            .copied()
            .unwrap_or("ready");
        by_status.entry(key).or_default().push(story);
    }

    let all_stories_points_width = story_points_column_width(stories.iter());

    let mut has_previous_section = false;
    for status in status_order {
        let Some(bucket) = by_status.get(status) else {
            continue;
        };
        if bucket.is_empty() {
            continue;
        }
        if has_previous_section {
            push_line(&mut output, "");
        }
        has_previous_section = true;

        let icon_label = theme.status_text(status, format!("{} {status}", status_icon(status)));
        let bucket_points = sum_story_points(bucket.iter().copied());
        let points_label = theme.story_points(format_story_points(bucket_points));
        let story_count = format_story_count(bucket.len());
        push_inset_line(
            &mut output,
            &format!(
                "{icon_label}   {}   {points_label}",
                theme.count(story_count)
            ),
        );

        let stories_ref: Vec<StoryOverview> = bucket.iter().map(|&s| s.clone()).collect();
        push_story_table(
            &mut output,
            theme,
            story_table_width,
            &stories_ref,
            all_stories_points_width,
        );
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

    let mut rendered_group = false;
    for status in ["todo", "in-progress", "blocked", "done"] {
        let tasks = details
            .tasks
            .iter()
            .filter(|task| task.normalized_status == status)
            .collect::<Vec<_>>();
        if tasks.is_empty() {
            continue;
        }

        if rendered_group {
            push_line(output, "");
        }
        rendered_group = true;
        push_line(
            output,
            &format!(
                "  {}",
                theme.status_text(status, format!("{} {status}", status_icon(status)))
            ),
        );
        for task in tasks {
            push_story_task_row(output, theme, layout.width, task);
        }
    }
}

pub(crate) fn push_story_task_row(output: &mut String, theme: &Theme, width: usize, task: &Task) {
    let summary = format_task_row_summary(task);
    let row_prefix = SPRINT_STORY_ROW_PREFIX;
    let leading_prefix_plain = format!("{row_prefix}{}  ", task.id);
    let leading_prefix = format!("{row_prefix}{}  ", theme.id(&task.id));
    let continuation_prefix = " ".repeat(display_width(&leading_prefix_plain));
    let summary_width = width
        .saturating_sub(display_width(&leading_prefix_plain))
        .max(1);

    for (index, line) in wrap_text(&summary, summary_width).iter().enumerate() {
        let prefix = if index == 0 {
            leading_prefix.as_str()
        } else {
            continuation_prefix.as_str()
        };
        push_line(output, &format!("{prefix}{line}"));
    }

    let description = task.description.trim();
    if description.is_empty() {
        return;
    }

    let description_prefix = " ".repeat(display_width(SPRINT_STORY_ROW_PREFIX) + 2);
    let description_width = width
        .saturating_sub(display_width(&description_prefix))
        .max(1);
    for line in wrap_text(description, description_width) {
        push_line(
            output,
            &format!("{description_prefix}{}", theme.paint(Style::Muted, line)),
        );
    }
}

pub(crate) fn format_task_row_summary(task: &Task) -> String {
    if task.tags.is_empty() {
        task.title.clone()
    } else {
        format!("{}  [{}]", task.title, task.tags.join(", "))
    }
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
            planned_start: None,
            planned_end: None,
        }];

        let output = render_story_list(
            &theme,
            OutputLayout { width: 80 },
            "active sprint (S000.getting-started)",
            &stories,
        );

        assert!(output.contains("Stories: 1"));
        assert!(output.contains("Scope: active sprint (S000.getting-started)"));
        assert!(output.contains("US-F1-010 ◈3"));
        assert!(output.contains("CI pipeline"));
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
                planned_start: None,
                planned_end: None,
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
        assert!(output.contains("  → in-progress"));
        assert!(output.contains("    · TASK-US-F1-010-001  Build story renderer [cli]"));
        assert!(output.contains("        Wire command output"));
        assert!(!output.contains("Story:"));
        assert!(!output.contains("Task file"));
        assert!(!output.contains("delivery/backlog/"));
        assert!(!output.contains("| Risk | Mitigation |"));
        assert!(!output.contains("- [ ] Run `cargo test`"));
    }

    #[test]
    fn story_details_wrap_task_rows_to_terminal_width() {
        let theme = Theme::plain();
        let details = StoryDetails {
            story: StoryOverview {
                id: "US-F1-999".to_string(),
                title: "Compact task rendering".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F1-99".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Ada Lovelace <ada@example.test>".to_string(),
                story_points: "5".to_string(),
                sprint: Some("S999.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-999.md"),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: None,
                planned_start: None,
                planned_end: None,
            },
            story_file_path: PathBuf::from("delivery/backlog/phase-1/US-F1-999.md"),
            task_file_path: None,
            epic_id: Some("EP-F1-99".to_string()),
            epic_title: Some("CLI".to_string()),
            work_started: None,
            work_done: None,
            story_statement: None,
            acceptance_criteria: None,
            definition_of_done: None,
            notes_and_open_questions: None,
            tasks: vec![kanban_core::Task {
                id: "TASK-US-F1-999-001".to_string(),
                title: "Improve the task renderer so compact task rows wrap responsively inside narrow terminals"
                    .to_string(),
                status: "In Progress".to_string(),
                normalized_status: "in-progress".to_string(),
                tags: vec!["cli".to_string(), "ux".to_string()],
                description: "Keep task descriptions readable without falling back to the old table layout."
                    .to_string(),
            }],
        };

        let output = render_story_details(&theme, OutputLayout { width: 80 }, &details);

        assert!(output.contains("  → in-progress"));
        assert!(output.contains("TASK-US-F1-999-001"));
        assert!(output.contains("[cli, ux]"));
        assert!(!output.contains("Task  Status"));
        assert!(!output.contains("Status  Tags"));
        for line in output.lines() {
            assert!(
                display_width(line) <= 80,
                "line exceeded 80 columns: {line}"
            );
        }
    }
}
