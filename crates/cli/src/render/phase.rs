#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn print_phase_overview(theme: &Theme, layout: OutputLayout, phase: &PhaseOverview) {
    print!("{}", render_phase_overview(theme, layout, phase));
}

pub(crate) fn render_phase_overview(
    theme: &Theme,
    layout: OutputLayout,
    phase: &PhaseOverview,
) -> String {
    let mut output = String::new();
    let grouped = phase_stories_by_epic(phase);
    let story_count = phase.stories.len();
    let drafted_points = phase_story_points_for_statuses(phase, &["draft", "ready"]);
    let planned_points = phase_story_points_for_statuses(phase, &["todo"]);
    let in_progress_points =
        phase_story_points_for_statuses(phase, &["in-progress", "ready-for-qa", "blocked"]);
    let done_points = phase_story_points_for_statuses(phase, &["done"]);
    let total_points = drafted_points + planned_points + in_progress_points + done_points;
    let summary = PhaseHeaderSummary {
        story_count,
        epic_count: grouped.len(),
        drafted_points,
        planned_points,
        in_progress_points,
        done_points,
        total_points,
    };

    push_phase_header_band(&mut output, theme, layout, phase, &summary);

    let points_width = story_points_column_width(phase.stories.iter());
    for (index, (epic_label, stories)) in grouped.iter().enumerate() {
        if index > 0 {
            push_line(&mut output, "");
        }

        let epic_points = sum_story_points(stories.iter().copied());
        push_line(
            &mut output,
            &format!(
                "{}   {}   {}",
                theme.heading(epic_label),
                theme.count(format_story_count(stories.len())),
                theme.story_points(format_story_points(epic_points)),
            ),
        );

        let stories_by_status = phase_stories_by_status(stories);
        for status in phase_status_display_order() {
            let Some(status_stories) = stories_by_status.get(status) else {
                continue;
            };

            push_line(&mut output, "");
            let icon_label = theme.status_text(status, format!("{} {status}", status_icon(status)));
            let status_points = sum_story_points(status_stories.iter().copied());
            push_line(
                &mut output,
                &format!(
                    "{}   {}   {}",
                    icon_label,
                    theme.count(format_story_count(status_stories.len())),
                    theme.story_points(format_story_points(status_points)),
                ),
            );
            push_phase_story_table(
                &mut output,
                theme,
                layout.width,
                status_stories,
                points_width,
            );
        }
    }

    output
}

pub(crate) struct PhaseHeaderSummary {
    pub(crate) story_count: usize,
    pub(crate) epic_count: usize,
    pub(crate) drafted_points: usize,
    pub(crate) planned_points: usize,
    pub(crate) in_progress_points: usize,
    pub(crate) done_points: usize,
    pub(crate) total_points: usize,
}

pub(crate) fn push_phase_header_band(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    phase: &PhaseOverview,
    summary: &PhaseHeaderSummary,
) {
    let prefix_text = format!("─── {} · Phase Overview ", phase.phase);
    let suffix_text = " ───";
    let fill = layout
        .width
        .saturating_sub(display_width(&prefix_text) + display_width(suffix_text));
    push_line(
        output,
        &format!(
            "{}{}",
            theme.paint(Style::Muted, prefix_text),
            theme.paint(Style::Muted, format!("{}{}", "─".repeat(fill), suffix_text)),
        ),
    );
    push_line(
        output,
        &format!(
            "  {}  {}",
            theme.label("Scope:"),
            theme.paint(
                Style::Muted,
                format!("phase backlog grouped by epic ({})", summary.epic_count)
            )
        ),
    );

    let bar = render_progress_bar(
        theme,
        summary.done_points,
        summary.in_progress_points,
        summary.total_points,
        layout.width,
    );
    let pct = summary
        .done_points
        .checked_mul(100)
        .and_then(|value| value.checked_div(summary.total_points))
        .unwrap_or(0);
    let progress_points = format!(
        "{} / {}",
        format_story_points(summary.done_points),
        summary.total_points
    );
    push_line(
        output,
        &format!(
            "  {}  {}  {}",
            theme.label("Progress:"),
            bar,
            format_args!(
                "{}  {}",
                theme.story_points(progress_points),
                theme.paint(Style::Muted, format!("{pct}%"))
            )
        ),
    );

    let dot = theme.paint(Style::Muted, "·");
    let segments = [
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.drafted_points)),
            theme.paint(Style::Yellow, "drafted")
        ),
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.planned_points)),
            theme.paint(Style::Muted, "planned")
        ),
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.in_progress_points)),
            theme.paint(Style::Blue, "in progress")
        ),
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.done_points)),
            theme.paint(Style::Green, "done")
        ),
    ];
    push_line(
        output,
        &format!("  {}", segments.join(&format!("  {dot}  "))),
    );

    push_line(
        output,
        &format!(
            "  {}  {}  {}",
            theme.count(format_story_count(summary.story_count)),
            theme.paint(Style::Muted, format_epic_count(summary.epic_count)),
            theme.story_points(format!(
                "{} total",
                format_story_points(summary.total_points)
            )),
        ),
    );
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

pub(crate) fn phase_stories_by_epic<'a>(
    phase: &'a PhaseOverview,
) -> Vec<(String, Vec<&'a StoryOverview>)> {
    let mut grouped: BTreeMap<String, Vec<&'a StoryOverview>> = BTreeMap::new();
    for story in &phase.stories {
        let label = story_epic_label(story.epic_id.as_deref(), story.epic_title.as_deref())
            .unwrap_or_else(|| "No epic".to_string());
        grouped.entry(label).or_default().push(story);
    }
    grouped.into_iter().collect()
}

pub(crate) fn phase_stories_by_status<'a>(
    stories: &[&'a StoryOverview],
) -> BTreeMap<&'a str, Vec<&'a StoryOverview>> {
    let mut grouped: BTreeMap<&'a str, Vec<&'a StoryOverview>> = BTreeMap::new();
    for story in stories {
        grouped
            .entry(story.status.as_str())
            .or_default()
            .push(*story);
    }
    grouped
}

pub(crate) fn phase_status_display_order() -> &'static [&'static str] {
    &[
        "draft",
        "ready",
        "todo",
        "in-progress",
        "ready-for-qa",
        "blocked",
        "done",
        "dropped",
    ]
}

pub(crate) fn phase_story_points_for_statuses(phase: &PhaseOverview, statuses: &[&str]) -> usize {
    phase
        .stories
        .iter()
        .filter(|story| statuses.contains(&story.status.as_str()))
        .map(|story| parse_story_points(&story.story_points))
        .sum()
}

pub(crate) fn format_epic_count(count: usize) -> String {
    if count == 1 {
        "1 epic".to_string()
    } else {
        format!("{count} epics")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_overview_groups_stories_by_epic_and_status() {
        let theme = Theme::plain();
        let phase = PhaseOverview {
            phase: "F1".to_string(),
            stories: vec![
                StoryOverview {
                    id: "US-F1-010".to_string(),
                    title: "CI pipeline with build and unit tests".to_string(),
                    status: "todo".to_string(),
                    epic_id: Some("EP-F1-01".to_string()),
                    epic_title: Some("Platform".to_string()),
                    assignee: "Ada Lovelace <ada@example.test>".to_string(),
                    story_points: "3".to_string(),
                    sprint: Some("S000.getting-started".to_string()),
                    relative_path: PathBuf::from(
                        "delivery/backlog/phase-1-scaffolding/01.platform/US-F1-010.md",
                    ),
                    task_summary: Some(TaskSummary {
                        todo: 2,
                        in_progress: 0,
                        blocked: 0,
                        done: 1,
                    }),
                    task_count: 3,
                    work_started: None,
                    work_done: None,
                    planned_start: None,
                    planned_end: None,
                },
                StoryOverview {
                    id: "US-F1-011".to_string(),
                    title: "Preview story details in the terminal".to_string(),
                    status: "in-progress".to_string(),
                    epic_id: Some("EP-F1-01".to_string()),
                    epic_title: Some("Platform".to_string()),
                    assignee: "Grace Hopper <grace@example.test>".to_string(),
                    story_points: "5".to_string(),
                    sprint: None,
                    relative_path: PathBuf::from(
                        "delivery/backlog/phase-1-scaffolding/01.platform/US-F1-011.md",
                    ),
                    task_summary: Some(TaskSummary {
                        todo: 1,
                        in_progress: 2,
                        blocked: 0,
                        done: 0,
                    }),
                    task_count: 3,
                    work_started: None,
                    work_done: None,
                    planned_start: None,
                    planned_end: None,
                },
                StoryOverview {
                    id: "US-F1-020".to_string(),
                    title: "Sync sprint rosters from story metadata".to_string(),
                    status: "done".to_string(),
                    epic_id: Some("EP-F1-02".to_string()),
                    epic_title: Some("Planning".to_string()),
                    assignee: "TBD".to_string(),
                    story_points: "2".to_string(),
                    sprint: Some("S001.foundation".to_string()),
                    relative_path: PathBuf::from(
                        "delivery/backlog/phase-1-scaffolding/02.planning/US-F1-020.md",
                    ),
                    task_summary: None,
                    task_count: 0,
                    work_started: None,
                    work_done: None,
                    planned_start: None,
                    planned_end: None,
                },
            ],
        };

        let output = render_phase_overview(&theme, OutputLayout { width: 100 }, &phase);

        assert!(output.contains("F1 · Phase Overview"));
        assert!(output.contains("3 stories"));
        assert!(output.contains("Progress:"));
        assert!(output.contains("◈2 / 10"));
        assert!(output.contains("20%"));
        assert!(output.contains("◈0 drafted"));
        assert!(output.contains("◈3 planned"));
        assert!(output.contains("◈5 in progress"));
        assert!(output.contains("◈2 done"));
        assert!(output.contains("2 epics"));
        assert!(output.contains("◈10 total"));
        assert!(output.contains("EP-F1-01  Platform   2 stories   ◈8"));
        assert!(output.contains("○ todo   1 story   ◈3"));
        assert!(output.contains("→ in-progress   1 story   ◈5"));
        assert!(output.contains("✓ done   1 story   ◈2"));
        assert!(output.contains("S000.getting-started"));
        assert!(output.contains("~"));
        assert!(output.contains("Ada Lovelace"));
        assert!(output.contains("Grace Hopper"));
        assert!(output.contains("Sync sprint rosters from story metadata"));
        for line in output.lines() {
            assert!(
                display_width(line) <= 100,
                "line exceeded 100 columns: {line}"
            );
        }
    }
}
