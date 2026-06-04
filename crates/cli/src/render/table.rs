#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

#[derive(Copy, Clone)]
pub(crate) enum CellStyle {
    Id,
    Warning,
    Precolored,
}

pub(crate) struct TableCell {
    pub(crate) text: String,
    pub(crate) style: Option<CellStyle>,
    pub(crate) preformatted: bool,
}

pub(crate) struct DynamicTableColumn {
    pub(crate) title: String,
    pub(crate) width: usize,
}

impl TableCell {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            preformatted: false,
        }
    }

    pub(crate) fn styled(text: impl Into<String>, style: CellStyle) -> Self {
        Self {
            text: text.into(),
            style: Some(style),
            preformatted: false,
        }
    }

    pub(crate) fn preformatted(text: impl Into<String>, style: CellStyle) -> Self {
        Self {
            text: text.into(),
            style: Some(style),
            preformatted: true,
        }
    }
}

pub(crate) fn push_story_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    stories: &[StoryOverview],
    points_width: usize,
) {
    let columns = story_table_columns(width, stories, points_width);
    let rows = stories
        .iter()
        .map(|story| {
            vec![
                TableCell::preformatted(
                    format_colored_story_status_label(theme, story, points_width),
                    CellStyle::Precolored,
                ),
                TableCell::new(&story.title),
                TableCell::new(extract_assignee_name(&story.assignee)),
                TableCell::styled(
                    format_colored_task_summary(theme, story.task_summary.as_ref()),
                    CellStyle::Precolored,
                ),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_story_rows(output, theme, &columns, &rows);
}

pub(crate) fn push_phase_story_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    stories: &[&StoryOverview],
    points_width: usize,
) {
    let columns = phase_story_table_columns(width, stories, points_width);
    let rows = stories
        .iter()
        .map(|story| {
            vec![
                TableCell::preformatted(
                    format_colored_story_status_label(theme, story, points_width),
                    CellStyle::Precolored,
                ),
                TableCell::new(&story.title),
                TableCell::new(story.sprint.as_deref().unwrap_or("~")),
                TableCell::new(extract_assignee_name(&story.assignee)),
                TableCell::styled(
                    format_colored_task_summary(theme, story.task_summary.as_ref()),
                    CellStyle::Precolored,
                ),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_rows(output, theme, &columns, &rows);
}

pub(crate) fn push_blocked_work_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    items: &[kanban_core::BlockedWorkItem],
) {
    let columns = blocked_work_table_columns(width, items);
    let rows = items
        .iter()
        .map(|item| {
            vec![
                TableCell::styled(&item.story_id, CellStyle::Id),
                TableCell::new(&item.story_title),
                TableCell::styled(
                    item.task_id.clone().unwrap_or_else(|| "-".to_string()),
                    CellStyle::Warning,
                ),
                TableCell::new(item.task_title.clone().unwrap_or_else(|| "-".to_string())),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_rows(output, theme, &columns, &rows);
}

pub(crate) fn story_table_columns(
    width: usize,
    stories: &[StoryOverview],
    points_width: usize,
) -> Vec<(&'static str, usize)> {
    let available = row_content_width(width, 4);
    let id_width = stories
        .iter()
        .map(|story| display_width(&format_story_status_label(story, points_width)))
        .max()
        .unwrap_or(5)
        .clamp(5, 18);
    let task_width = stories
        .iter()
        .map(|story| display_width(&format_compact_task_summary(story.task_summary.as_ref())))
        .max()
        .unwrap_or(5)
        .clamp(5, 17);
    let raw_assignee_width = stories
        .iter()
        .map(|story| display_width(extract_assignee_name(&story.assignee)))
        .max()
        .unwrap_or(8)
        .max(8);
    // Clamp assignee so title always gets at least 20 columns.
    let max_assignee = available.saturating_sub(id_width + task_width + 20);
    let assignee_width = raw_assignee_width.min(max_assignee.max(8));
    let title_width = available
        .saturating_sub(id_width + assignee_width + task_width)
        .max(1);

    vec![
        ("Story", id_width),
        ("Description", title_width),
        ("Assignee", assignee_width),
        ("Tasks", task_width),
    ]
}

pub(crate) fn phase_story_table_columns(
    width: usize,
    stories: &[&StoryOverview],
    points_width: usize,
) -> Vec<(&'static str, usize)> {
    let available = row_content_width(width, 5);
    let id_width = stories
        .iter()
        .map(|story| display_width(&format_story_status_label(story, points_width)))
        .max()
        .unwrap_or(5)
        .clamp(5, 18);
    let sprint_width = stories
        .iter()
        .map(|story| display_width(story.sprint.as_deref().unwrap_or("~")))
        .max()
        .unwrap_or(1)
        .clamp(1, 22);
    let task_width = stories
        .iter()
        .map(|story| display_width(&format_compact_task_summary(story.task_summary.as_ref())))
        .max()
        .unwrap_or(5)
        .clamp(5, 17);
    let raw_assignee_width = stories
        .iter()
        .map(|story| display_width(extract_assignee_name(&story.assignee)))
        .max()
        .unwrap_or(8)
        .max(8);
    let max_assignee = available.saturating_sub(id_width + sprint_width + task_width + 20);
    let assignee_width = raw_assignee_width.min(max_assignee.max(8));
    let title_width = available
        .saturating_sub(id_width + sprint_width + assignee_width + task_width)
        .max(1);

    vec![
        ("Story", id_width),
        ("Description", title_width),
        ("Sprint", sprint_width),
        ("Assignee", assignee_width),
        ("Tasks", task_width),
    ]
}

pub(crate) fn blocked_work_table_columns(
    width: usize,
    items: &[kanban_core::BlockedWorkItem],
) -> Vec<(&'static str, usize)> {
    let available = row_content_width(width, 4);
    let story_width = items
        .iter()
        .map(|item| display_width(&item.story_id))
        .max()
        .unwrap_or(5)
        .clamp(5, 12);
    let task_width = items
        .iter()
        .filter_map(|item| item.task_id.as_deref())
        .map(display_width)
        .max()
        .unwrap_or(4)
        .clamp(4, 10);
    let remaining = available.saturating_sub(story_width + task_width);
    let story_title_width = remaining / 2;
    let task_title_width = remaining.saturating_sub(story_title_width);

    vec![
        ("Story", story_width),
        ("Description", story_title_width.max(16)),
        ("Task", task_width),
        ("Task description", task_title_width.max(16)),
    ]
}

pub(crate) fn row_content_width(width: usize, column_count: usize) -> usize {
    let indent = 2;
    let gaps = column_count.saturating_sub(1) * 2;
    width.saturating_sub(indent + gaps)
}

pub(crate) fn push_wrapped_rows(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    rows: &[Vec<TableCell>],
) {
    for row in rows {
        push_wrapped_table_row(output, theme, columns, row);
    }
}

pub(crate) fn push_wrapped_story_rows(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    rows: &[Vec<TableCell>],
) {
    for row in rows {
        push_wrapped_story_table_row(output, theme, columns, row);
    }
}

pub(crate) fn push_wrapped_table(
    output: &mut String,
    theme: &Theme,
    columns: &[DynamicTableColumn],
    rows: &[Vec<TableCell>],
) {
    let header = columns
        .iter()
        .map(|column| TableCell::preformatted(theme.label(&column.title), CellStyle::Precolored))
        .collect::<Vec<_>>();
    push_wrapped_dynamic_table_row(output, theme, columns, &header);

    let mut separator = String::from("  ");
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            separator.push_str("  ");
        }
        separator.push_str(&theme.paint(Style::Muted, "─".repeat(column.width)));
    }
    push_line(output, &separator);

    for row in rows {
        push_wrapped_dynamic_table_row(output, theme, columns, row);
    }
}

pub(crate) fn push_wrapped_dynamic_table_row(
    output: &mut String,
    theme: &Theme,
    columns: &[DynamicTableColumn],
    row: &[TableCell],
) {
    let wrapped_cells = row
        .iter()
        .zip(columns)
        .map(|(cell, column)| {
            if cell.preformatted {
                vec![cell.text.clone()]
            } else {
                wrap_text(&cell.text, column.width)
            }
        })
        .collect::<Vec<_>>();
    let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);

    for line_index in 0..row_height {
        let mut line = String::new();
        line.push_str("  ");
        for ((cell, column), wrapped) in row.iter().zip(columns).zip(&wrapped_cells) {
            let value = wrapped.get(line_index).map(String::as_str).unwrap_or("");
            let padded = pad_to_width(value, column.width);
            if line.len() > 2 {
                line.push_str("  ");
            }
            line.push_str(&style_table_cell(theme, cell.style, &padded));
        }
        push_line(output, &line);
    }
}

pub(crate) fn push_wrapped_table_row(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    row: &[TableCell],
) {
    let wrapped_cells = row
        .iter()
        .zip(columns)
        .map(|(cell, (_, width))| {
            if cell.preformatted {
                vec![cell.text.clone()]
            } else {
                wrap_text(&cell.text, *width)
            }
        })
        .collect::<Vec<_>>();
    let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);

    for line_index in 0..row_height {
        let mut line = String::new();
        line.push_str("  ");
        for ((cell, (_, width)), wrapped) in row.iter().zip(columns).zip(&wrapped_cells) {
            let value = wrapped.get(line_index).map(String::as_str).unwrap_or("");
            let padded = pad_to_width(value, *width);
            if line.len() > 2 {
                line.push_str("  ");
            }
            line.push_str(&style_table_cell(theme, cell.style, &padded));
        }
        push_line(output, &line);
    }
}

pub(crate) fn push_wrapped_story_table_row(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    row: &[TableCell],
) {
    let wrapped_cells = row
        .iter()
        .zip(columns)
        .map(|(cell, (_, width))| {
            if cell.preformatted {
                vec![cell.text.clone()]
            } else {
                wrap_text(&cell.text, *width)
            }
        })
        .collect::<Vec<_>>();
    let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);

    for line_index in 0..row_height {
        let prefix = if line_index == 0 {
            SPRINT_STORY_ROW_PREFIX.to_string()
        } else {
            " ".repeat(display_width(SPRINT_STORY_ROW_PREFIX))
        };
        let mut line = prefix;
        let prefix_width = display_width(&line);
        for ((cell, (_, width)), wrapped) in row.iter().zip(columns).zip(&wrapped_cells) {
            let value = wrapped.get(line_index).map(String::as_str).unwrap_or("");
            let padded = pad_to_width(value, *width);
            if display_width(&line) > prefix_width {
                line.push_str("  ");
            }
            line.push_str(&style_table_cell(theme, cell.style, &padded));
        }
        push_line(output, &line);
    }
}

pub(crate) fn style_table_cell(theme: &Theme, style: Option<CellStyle>, value: &str) -> String {
    match style {
        Some(CellStyle::Id) => theme.id(value),
        Some(CellStyle::Warning) => theme.warning(value),
        Some(CellStyle::Precolored) => value.to_string(),
        None => value.to_string(),
    }
}

pub(crate) fn pad_to_width(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(display_width(value));
    format!("{value}{}", " ".repeat(padding))
}

pub(crate) fn display_width(value: &str) -> usize {
    let mut count = 0;
    let mut in_escape = false;
    for ch in value.chars() {
        match ch {
            '\x1b' => in_escape = true,
            'm' if in_escape => in_escape = false,
            _ if !in_escape => count += 1,
            _ => {}
        }
    }
    count
}

pub(crate) fn wrap_text(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in value.split_whitespace() {
        if current.is_empty() {
            push_word_wrapped(&mut lines, &mut current, word, width);
        } else if display_width(&current) + 1 + display_width(word) <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            push_word_wrapped(&mut lines, &mut current, word, width);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(crate) fn push_word_wrapped(
    lines: &mut Vec<String>,
    current: &mut String,
    word: &str,
    width: usize,
) {
    let mut chunk = String::new();
    for character in word.chars() {
        if display_width(&chunk) == width {
            lines.push(std::mem::take(&mut chunk));
        }
        chunk.push(character);
    }
    *current = chunk;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_ignores_ansi_codes() {
        assert_eq!(display_width("\x1b[1;32mhello\x1b[0m"), 5);
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("\x1b[2m✓4\x1b[0m"), 2);
        assert_eq!(display_width(""), 0);
    }
}
