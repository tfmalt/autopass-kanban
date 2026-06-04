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
