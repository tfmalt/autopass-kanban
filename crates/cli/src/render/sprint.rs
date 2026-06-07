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
        push_inset_line(&mut output, &theme.heading("Sprint Goal:"));
        push_terminal_markdown_indented(&mut output, theme, layout.width, goal, 0);
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
            push_wrapped_hanging_line_inset(&mut output, "", warning, layout.width, |v| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_overview_wraps_story_rows_to_terminal_width() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F1-999".to_string(),
                title: "Improve current sprint terminal rendering so story descriptions wrap responsively inside the detected table boundary".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F1-99".to_string()),
                epic_title: Some("Terminal Rendering".to_string()),
                assignee: "Ada Lovelace <ada@example.test>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S999.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-9-test/US-F1-999.md"),
                task_summary: Some(TaskSummary {
                    todo: 1,
                    in_progress: 2,
                    blocked: 3,
                    done: 4,
                }),
                task_count: 10,
                work_started: None,
                work_done: None,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S999.test".to_string(),
            headline: "terminal-wrapping".to_string(),
            sprint_goal: Some(
                "Keep sprint output useful without repeating implementation file paths.\n\n- Highlight **important** work\n- Keep lines short".to_string(),
            ),
            start_date: "2026-05-29".to_string(),
            end_date: "2026-06-12".to_string(),
            readme_path: PathBuf::from("delivery/sprints/S999.test.md"),
            readme_status: Some("active".to_string()),
            stories_by_status,
            blocked_work: vec![kanban_core::BlockedWorkItem {
                story_id: "US-F1-999".to_string(),
                story_title: "Improve current sprint terminal rendering so blocked work also wraps responsively".to_string(),
                task_id: Some("T-001".to_string()),
                task_title: Some("Verify narrow but supported terminal widths do not overflow".to_string()),
            }],
            warnings: Vec::new(),
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 80 }, &sprint);

        assert!(output.contains("S999 · Terminal Wrapping"));
        assert!(output.contains("Sprint Goal:"));
        assert!(output.contains("important"));
        assert!(!output.contains("README:"));
        assert!(output.contains("US-F1-999"));
        assert!(!output.contains('|'));
        for line in output.lines() {
            assert!(
                display_width(line) <= 80,
                "line exceeded 80 columns: {line}"
            );
        }
    }

    #[test]
    fn sprint_goal_renders_markdown_with_colors() {
        let theme = Theme::color();
        let sprint = SprintOverview {
            sprint_name: "S100.goal".to_string(),
            headline: "goal".to_string(),
            sprint_goal: Some(
                "Build **visible** progress\n\n## Focus\n- Deliver value\n- Keep the team aligned"
                    .to_string(),
            ),
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("delivery/sprints/S100.goal.md"),
            readme_status: Some("active".to_string()),
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 80 }, &sprint);

        assert!(output.contains("\x1b[1;36mSprint Goal:\x1b[0m"));
        assert!(output.contains("\x1b[1;35mvisible\x1b[0m"));
        assert!(output.contains("\x1b[1;36mFocus\x1b[0m"));
        assert!(output.contains("• Deliver value"));
        assert!(output.contains("• Keep the team aligned"));
    }

    #[test]
    fn header_band_fills_terminal_width() {
        let theme = Theme::plain();
        let sprint = SprintOverview {
            sprint_name: "S001.foundation".to_string(),
            headline: "foundation".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("delivery/sprints/S001.foundation.md"),
            readme_status: Some("active".to_string()),
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };

        for width in [80, 100, 120] {
            let mut output = String::new();
            push_sprint_header_band(&mut output, &theme, OutputLayout { width }, &sprint);
            let non_empty: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
            // First line = top separator, last line = bottom separator — both full-width.
            assert_eq!(
                display_width(non_empty[0]),
                width,
                "top separator at width {width}"
            );
            assert_eq!(
                display_width(non_empty[non_empty.len() - 1]),
                width,
                "bottom separator at width {width}"
            );
        }
    }

    #[test]
    fn sprint_progress_uses_story_points() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "done".to_string(),
            vec![StoryOverview {
                id: "US-F1-001".to_string(),
                title: "Completed high-value story".to_string(),
                status: "done".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("Test Epic".to_string()),
                assignee: "TBD".to_string(),
                story_points: "8".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-001.md"),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: None,
            }],
        );
        stories_by_status.insert(
            "todo".to_string(),
            vec![StoryOverview {
                id: "US-F1-002".to_string(),
                title: "Remaining smaller story".to_string(),
                status: "todo".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("Test Epic".to_string()),
                assignee: "TBD".to_string(),
                story_points: "2".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-002.md"),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: None,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);

        assert!(
            output.contains("◈8 / 10"),
            "progress line should use story points: {output}"
        );
        assert!(
            output.contains("80%"),
            "progress percentage should use story points: {output}"
        );
    }

    #[test]
    fn task_symbols_replace_old_format() {
        let summary = TaskSummary {
            todo: 2,
            in_progress: 1,
            blocked: 0,
            done: 4,
        };
        let plain = format_compact_task_summary(Some(&summary));
        assert!(plain.contains("✓4"), "done symbol missing: {plain}");
        assert!(plain.contains("▶1"), "active symbol missing: {plain}");
        assert!(plain.contains("·2"), "todo symbol missing: {plain}");
        assert!(plain.contains("✗0"), "blocked symbol missing: {plain}");
        assert!(!plain.contains("T:"), "old T: format present: {plain}");
        assert!(!plain.contains("IP:"), "old IP: format present: {plain}");
    }

    #[test]
    fn story_status_rows_include_story_points() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "todo".to_string(),
            vec![StoryOverview {
                id: "US-F1-062".to_string(),
                title: "A larger story".to_string(),
                status: "todo".to_string(),
                epic_id: Some("EP-F1-06".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "13".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-062.md"),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: None,
            }],
        );
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F3-001".to_string(),
                title: "A smaller story".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F3-01".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "5".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-3/US-F3-001.md"),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: None,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);

        assert!(
            output.contains("US-F1-062 ◈13"),
            "story row should include story points: {output}"
        );
        assert!(
            output.contains("    · US-F1-062 ◈13"),
            "story row should be indented below the status header and prefixed with a bullet: {output}"
        );
        assert!(
            output.contains("○ todo   1 story   ◈13"),
            "todo header should include story point total: {output}"
        );
        assert!(
            output.contains("→ in-progress   1 story   ◈5"),
            "in-progress header should include story point total: {output}"
        );
        assert!(
            output.contains("US-F3-001  ◈5"),
            "single-digit story points should be right-aligned: {output}"
        );
    }

    #[test]
    fn story_status_rows_highlight_story_points() {
        let theme = Theme::color();
        let story = StoryOverview {
            id: "US-F1-002".to_string(),
            title: "A story in progress".to_string(),
            status: "in-progress".to_string(),
            epic_id: Some("EP-F1-01".to_string()),
            epic_title: Some("CLI".to_string()),
            assignee: "Someone <s@example.com>".to_string(),
            story_points: "3".to_string(),
            sprint: Some("S001.test".to_string()),
            relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-002.md"),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: None,
        };

        let label = format_colored_story_status_label(&theme, &story, 3);

        assert!(label.contains("\x1b[1;36mUS-F1-002\x1b[0m"));
        assert!(label.contains(" \x1b[1;33m◈3\x1b[0m"));
    }

    #[test]
    fn done_section_expands_in_overview() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "done".to_string(),
            vec![StoryOverview {
                id: "US-F1-001".to_string(),
                title: "A completed story".to_string(),
                status: "done".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "2".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-001.md"),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: None,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };
        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);
        assert!(
            output.contains("✓ done   1 story   ◈2"),
            "done section header missing story points"
        );
        assert!(
            output.contains("A completed story"),
            "done story should be listed individually"
        );
    }

    #[test]
    fn zero_count_section_shows_single_muted_line() {
        let theme = Theme::plain();
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };
        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);
        assert!(output.contains("○ todo"), "todo section header missing");
        assert!(
            output
                .lines()
                .any(|line| line == "  ○ todo   0 stories   ◈0   · none"),
            "todo section should be inset by two spaces"
        );
        assert!(
            output.contains("none"),
            "none placeholder missing for empty section"
        );
    }

    #[test]
    fn sprint_sections_are_divided_and_inset() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F1-002".to_string(),
                title: "A story in progress".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-002.md"),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: None,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: Some("Keep the overview readable.".to_string()),
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec!["A warning line".to_string()],
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);
        let divider = "─".repeat(100);

        assert!(
            output.lines().any(|line| line == divider),
            "section divider should span the full width without indentation"
        );
        assert!(
            output.lines().any(|line| line == "  A warning line"),
            "warning should be inset by two spaces"
        );
        assert!(
            output
                .lines()
                .any(|line| line == "  → in-progress   1 story   ◈3"),
            "status header should be inset by two spaces"
        );
    }

    #[test]
    fn sprint_header_title_uses_bright_color() {
        let theme = Theme::color();
        let sprint = SprintOverview {
            sprint_name: "S001.scaffolding".to_string(),
            headline: "scaffolding".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview_short(&theme, OutputLayout { width: 100 }, &sprint);

        assert!(
            output.contains("\x1b[1;36mS001 · Scaffolding\x1b[0m"),
            "sprint title should be highlighted with bright cyan: {output:?}"
        );
    }

    #[test]
    fn sprint_header_band_has_blank_lines_around_status_rows() {
        let theme = Theme::plain();
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview_short(&theme, OutputLayout { width: 100 }, &sprint);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 6, "header should only contain the header band");
        assert!(
            lines[1].is_empty(),
            "blank line should appear above the status rows"
        );
        assert!(
            lines[4].is_empty(),
            "blank line should appear below the status rows"
        );
    }
}
