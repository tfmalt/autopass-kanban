#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn print_sprint_overview(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) {
    print!("{}", render_sprint_overview(theme, layout, sprint));
}

pub(crate) fn print_sprint_overview_short(
    theme: &Theme,
    layout: OutputLayout,
    sprint: &SprintOverview,
) {
    print!("{}", render_sprint_overview_short(theme, layout, sprint));
}

pub(crate) fn render_sprint_overview(
    theme: &Theme,
    layout: OutputLayout,
    sprint: &SprintOverview,
) -> String {
    let mut output = String::new();
    let content_width = sprint_content_width(layout.width);
    let story_table_width = sprint_story_table_width(layout.width);
    let blocked_table_width = sprint_table_width(layout.width);
    let mut has_content_section = false;

    // Dashboard header band: top separator, progress line, count line, bottom separator
    push_sprint_header_band(&mut output, theme, layout, sprint);

    // Sprint goal (below bottom separator)
    if let Some(goal) = &sprint.sprint_goal {
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        push_wrapped_label_value_inset(&mut output, theme, "Sprint Goal:", goal, content_width);
    }

    // Warnings
    if !sprint.warnings.is_empty() {
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        for warning in &sprint.warnings {
            push_wrapped_hanging_line_inset(&mut output, "", warning, content_width, |v| {
                theme.warning(v)
            });
        }
    }

    // Status sections expanded with story rows.
    for status in ["todo", "in-progress", "ready-for-qa", "done"] {
        let stories = sprint
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
            let points_width =
                story_points_column_width(sprint.stories_by_status.values().flat_map(|v| v.iter()));
            push_story_table(&mut output, theme, story_table_width, stories, points_width);
        }
    }

    // Summary footer: ✗ blocked N
    let blocked_count = sprint
        .stories_by_status
        .get("blocked")
        .map(|v| v.len())
        .unwrap_or(0);
    let blocked_points = sprint
        .stories_by_status
        .get("blocked")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);
    push_sprint_section_divider_before_next(
        &mut output,
        theme,
        layout.width,
        &mut has_content_section,
    );
    let blocked_style = if blocked_count > 0 {
        Style::Red
    } else {
        Style::Muted
    };
    let blocked_part = theme.paint(blocked_style, format!("{} blocked", status_icon("blocked")));
    push_inset_line(
        &mut output,
        &format!(
            "{}   {}   {}",
            blocked_part,
            theme.count(format_story_count(blocked_count)),
            theme.story_points(format_story_points(blocked_points)),
        ),
    );

    // Blocked work detail callout
    push_sprint_section_divider_before_next(
        &mut output,
        theme,
        layout.width,
        &mut has_content_section,
    );
    push_inset_line(&mut output, &theme.heading("Blocked work"));
    if sprint.blocked_work.is_empty() {
        push_inset_line(&mut output, "- none");
    } else {
        push_blocked_work_table(
            &mut output,
            theme,
            blocked_table_width,
            &sprint.blocked_work,
        );
    }

    output
}

pub(crate) fn render_sprint_overview_short(
    theme: &Theme,
    layout: OutputLayout,
    sprint: &SprintOverview,
) -> String {
    let mut output = String::new();
    push_sprint_header_band(&mut output, theme, layout, sprint);
    output
}

pub(crate) fn title_case_headline(headline: &str) -> String {
    headline
        .split([' ', '-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn format_story_count(count: usize) -> String {
    if count == 1 {
        "1 story".to_string()
    } else {
        format!("{count} stories")
    }
}

pub(crate) fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

pub(crate) fn push_inset_line(output: &mut String, line: &str) {
    push_line(
        output,
        &format!("{}{}", " ".repeat(SPRINT_CONTENT_INSET), line),
    );
}

pub(crate) fn sprint_content_width(width: usize) -> usize {
    width.saturating_sub(SPRINT_CONTENT_INSET * 2).max(1)
}

pub(crate) fn sprint_table_width(width: usize) -> usize {
    width.saturating_sub(SPRINT_CONTENT_INSET).max(1)
}

pub(crate) fn sprint_story_table_width(width: usize) -> usize {
    width
        .saturating_sub(display_width(SPRINT_STORY_ROW_PREFIX))
        .max(1)
}

pub(crate) fn push_sprint_section_divider(output: &mut String, theme: &Theme, width: usize) {
    push_line(output, &theme.paint(Style::Muted, "─".repeat(width)));
}

pub(crate) fn push_sprint_section_divider_before_next(
    output: &mut String,
    theme: &Theme,
    width: usize,
    has_content_section: &mut bool,
) {
    if *has_content_section {
        push_sprint_section_divider(output, theme, width);
    }
    *has_content_section = true;
}

pub(crate) fn push_wrapped_label_value_inset(
    output: &mut String,
    theme: &Theme,
    label: &str,
    value: &str,
    width: usize,
) {
    let prefix_width = display_width(label) + 1;
    let value_width = width.saturating_sub(prefix_width).max(1);
    let wrapped = wrap_text(value, value_width);
    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            push_inset_line(output, &format!("{} {line}", theme.label(label)));
        } else {
            push_inset_line(output, &format!("{}{line}", " ".repeat(prefix_width)));
        }
    }
}

pub(crate) fn push_wrapped_hanging_line_inset(
    output: &mut String,
    prefix: &str,
    value: &str,
    width: usize,
    style: impl Fn(&str) -> String,
) {
    let value_width = width.saturating_sub(display_width(prefix)).max(1);
    let wrapped = wrap_text(value, value_width);
    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            push_inset_line(output, &format!("{prefix}{}", style(line)));
        } else {
            push_inset_line(
                output,
                &format!("{}{line}", " ".repeat(display_width(prefix))),
            );
        }
    }
}

pub(crate) fn push_wrapped_hanging_line(
    output: &mut String,
    prefix: &str,
    value: &str,
    width: usize,
    style: impl Fn(&str) -> String,
) {
    let value_width = width.saturating_sub(display_width(prefix)).max(1);
    let wrapped = wrap_text(value, value_width);
    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            push_line(output, &format!("{prefix}{}", style(line)));
        } else {
            push_line(
                output,
                &format!("{}{line}", " ".repeat(display_width(prefix))),
            );
        }
    }
}

pub(crate) fn format_colored_task_summary(theme: &Theme, summary: Option<&TaskSummary>) -> String {
    summary
        .map(|s| {
            format!(
                "{} {} {} {}",
                theme.paint(Style::Green, format!("✓{}", s.done)),
                theme.paint(Style::Blue, format!("▶{}", s.in_progress)),
                theme.paint(Style::Muted, format!("·{}", s.todo)),
                theme.paint(Style::Red, format!("✗{}", s.blocked)),
            )
        })
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn push_sprint_header_band(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    sprint: &SprintOverview,
) {
    let sprint_id = sprint
        .sprint_name
        .split_once('.')
        .map(|(id, _)| id)
        .unwrap_or(&sprint.sprint_name);
    let headline = title_case_headline(&sprint.headline);
    let status_label = sprint_status_label(&sprint.end_date, sprint.readme_status.as_deref());

    // Top separator: ─── S000 · Headline [fill] status ───
    let title_text = format!("{} · {}", sprint_id, headline);
    let prefix_text = format!("─── {title_text} ");
    let suffix_text = format!(" {} ───", status_label);
    let fill = layout
        .width
        .saturating_sub(display_width(&prefix_text) + display_width(&suffix_text));
    let colored_status = match status_label {
        "overdue" => theme.paint(Style::Yellow, status_label),
        "completed" => theme.paint(Style::Muted, status_label),
        _ => status_label.to_string(),
    };
    push_line(
        output,
        &format!(
            "{}{}{} {} {}",
            theme.paint(Style::Muted, "─── "),
            theme.paint(Style::Cyan, title_text),
            theme.paint(Style::Muted, format!(" {}", "─".repeat(fill))),
            colored_status,
            theme.paint(Style::Muted, "───"),
        ),
    );

    push_line(output, "");

    // Counts per status
    let total_points: usize = sprint
        .stories_by_status
        .values()
        .map(|stories| sum_story_points(stories.iter()))
        .sum();
    let done = sprint
        .stories_by_status
        .get("done")
        .map(|v| v.len())
        .unwrap_or(0);
    let done_points = sprint
        .stories_by_status
        .get("done")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);
    let in_progress = sprint
        .stories_by_status
        .get("in-progress")
        .map(|v| v.len())
        .unwrap_or(0);
    let qa = sprint
        .stories_by_status
        .get("ready-for-qa")
        .map(|v| v.len())
        .unwrap_or(0);
    let todo = sprint
        .stories_by_status
        .get("todo")
        .map(|v| v.len())
        .unwrap_or(0);
    let blocked = sprint
        .stories_by_status
        .get("blocked")
        .map(|v| v.len())
        .unwrap_or(0);
    let in_progress_points = sprint
        .stories_by_status
        .get("in-progress")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);

    // Progress line
    let bar = render_progress_bar(
        theme,
        done_points,
        in_progress_points,
        total_points,
        layout.width,
    );
    let pct = done_points
        .checked_mul(100)
        .and_then(|value| value.checked_div(total_points))
        .unwrap_or(0);
    push_line(
        output,
        &format!(
            "  {} → {}   {}  {}  {}",
            sprint.start_date,
            sprint.end_date,
            bar,
            theme.story_points(format!(
                "{} / {}",
                format_story_points(done_points),
                total_points
            )),
            theme.paint(Style::Muted, format!("{pct}%")),
        ),
    );

    // Count line: N done · N in progress · N in qa · N todo · N blocked
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

    // Bottom separator: full-width dashes
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

pub(crate) fn format_compact_task_summary(summary: Option<&TaskSummary>) -> String {
    summary
        .map(|s| format!("✓{} ▶{} ·{} ✗{}", s.done, s.in_progress, s.todo, s.blocked))
        .unwrap_or_else(|| "-".to_string())
}
