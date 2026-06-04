#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn extract_assignee_name(assignee: &str) -> &str {
    assignee
        .find('<')
        .map(|pos| assignee[..pos].trim())
        .unwrap_or_else(|| assignee.trim())
}

pub(crate) fn status_icon(status: &str) -> &'static str {
    match status {
        "todo" => "○",
        "in-progress" => "→",
        "ready-for-qa" => "◎",
        "done" => "✓",
        "blocked" => "✗",
        _ => "·",
    }
}

pub(crate) fn parse_story_points(story_points: &str) -> usize {
    story_points.trim().parse().unwrap_or(0)
}

pub(crate) fn format_story_points(value: impl std::fmt::Display) -> String {
    format!("◈{value}")
}

pub(crate) fn story_points_column_width<'a>(
    stories: impl IntoIterator<Item = &'a StoryOverview>,
) -> usize {
    stories
        .into_iter()
        .map(|story| display_width(&format_story_points(&story.story_points)))
        .max()
        .unwrap_or(0)
}

pub(crate) fn sum_story_points<'a>(stories: impl IntoIterator<Item = &'a StoryOverview>) -> usize {
    stories
        .into_iter()
        .map(|story| parse_story_points(&story.story_points))
        .sum()
}

pub(crate) fn format_story_status_label(story: &StoryOverview, points_width: usize) -> String {
    let points = format_story_points(&story.story_points);
    let padding = " ".repeat(points_width.saturating_sub(display_width(&points)));
    format!("{} {}{}", story.id, padding, points)
}

pub(crate) fn format_colored_story_status_label(
    theme: &Theme,
    story: &StoryOverview,
    points_width: usize,
) -> String {
    let points = format_story_points(&story.story_points);
    let padding = " ".repeat(points_width.saturating_sub(display_width(&points)));
    format!(
        "{} {}{}",
        theme.id(&story.id),
        padding,
        theme.story_points(points)
    )
}

pub(crate) fn sprint_status_label(end_date: &str, readme_status: Option<&str>) -> &'static str {
    if readme_status
        .map(|s| matches!(s, "completed" | "closed" | "done"))
        .unwrap_or(false)
    {
        return "completed";
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
        let today = chrono::Local::now().date_naive();
        if date >= today { "active" } else { "overdue" }
    } else {
        "active"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignee_strips_email() {
        assert_eq!(
            extract_assignee_name("Geir Ivar Jerstad <g@v.no>"),
            "Geir Ivar Jerstad"
        );
        assert_eq!(
            extract_assignee_name("Thomas Malt <thomas.malt@vegvesen.no>"),
            "Thomas Malt"
        );
        assert_eq!(
            extract_assignee_name("Sondre Bjerkerud and Erik Itland"),
            "Sondre Bjerkerud and Erik Itland"
        );
        assert_eq!(extract_assignee_name("TBD"), "TBD");
    }
}
