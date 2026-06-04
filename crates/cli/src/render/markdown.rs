#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn push_terminal_markdown(
    output: &mut String,
    theme: &Theme,
    width: usize,
    content: &str,
) {
    let mut table_lines = Vec::new();
    let mut code_block = CodeBlockKind::None;

    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if is_markdown_table_line(trimmed) && matches!(code_block, CodeBlockKind::None) {
            table_lines.push(trimmed.to_string());
            continue;
        }
        flush_markdown_table(output, theme, width, &mut table_lines);

        if trimmed.starts_with("```") {
            code_block = toggle_code_block(code_block, trimmed);
            continue;
        }

        if !matches!(code_block, CodeBlockKind::None) {
            push_code_block_line(output, theme, width, line, code_block);
            continue;
        }

        push_terminal_markdown_line(output, theme, width, line);
    }

    flush_markdown_table(output, theme, width, &mut table_lines);
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum CodeBlockKind {
    None,
    Plain,
    Gherkin,
}

pub(crate) fn toggle_code_block(current: CodeBlockKind, fence_line: &str) -> CodeBlockKind {
    if !matches!(current, CodeBlockKind::None) {
        return CodeBlockKind::None;
    }

    let info = fence_line.trim_start_matches('`').trim();
    if info.eq_ignore_ascii_case("gherkin") {
        CodeBlockKind::Gherkin
    } else {
        CodeBlockKind::Plain
    }
}

pub(crate) fn push_code_block_line(
    output: &mut String,
    theme: &Theme,
    width: usize,
    line: &str,
    code_block: CodeBlockKind,
) {
    match code_block {
        CodeBlockKind::Gherkin => push_gherkin_code_line(output, theme, width, line),
        CodeBlockKind::Plain => {
            push_wrapped_hanging_line(output, "  │ ", line, width, |value| theme.path(value));
        }
        CodeBlockKind::None => {}
    }
}

pub(crate) fn push_gherkin_code_line(output: &mut String, theme: &Theme, width: usize, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        push_line(output, "  │");
        return;
    }

    if let Some((keyword, rest)) = gherkin_line(trimmed) {
        push_wrapped_hanging_line(
            output,
            "  │ ",
            &format!("{} {}", theme.label(keyword), clean_inline_markdown(rest)),
            width,
            |value| value.to_string(),
        );
    } else {
        push_wrapped_hanging_line(output, "  │ ", trimmed, width, |value| theme.path(value));
    }
}

pub(crate) fn push_terminal_markdown_line(
    output: &mut String,
    theme: &Theme,
    width: usize,
    line: &str,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        push_line(output, "");
        return;
    }

    if let Some(heading) = markdown_heading_text(trimmed) {
        push_wrapped_hanging_line(
            output,
            "",
            &clean_inline_markdown(heading),
            width,
            |value| theme.heading(value),
        );
        return;
    }

    if let Some(quote) = trimmed.strip_prefix('>') {
        push_wrapped_hanging_line(
            output,
            "  │ ",
            &clean_inline_markdown(quote.trim()),
            width,
            |value| theme.path(value),
        );
        return;
    }

    if let Some((marker, value)) = markdown_list_item(trimmed) {
        push_wrapped_hanging_line(
            output,
            &format!("  {marker} "),
            &clean_inline_markdown(value),
            width,
            |value| value.to_string(),
        );
        return;
    }

    if let Some((keyword, rest)) = gherkin_line(trimmed) {
        push_wrapped_hanging_line(
            output,
            &format!("  {} ", theme.label(keyword)),
            &clean_inline_markdown(rest),
            width,
            |value| value.to_string(),
        );
        return;
    }

    push_wrapped_hanging_line(
        output,
        "  ",
        &clean_inline_markdown(trimmed),
        width,
        |value| value.to_string(),
    );
}

pub(crate) fn markdown_heading_text(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ') {
        Some(line[hashes + 1..].trim())
    } else {
        None
    }
}

pub(crate) fn markdown_list_item(line: &str) -> Option<(String, &str)> {
    for (prefix, marker) in [
        ("- [x] ", "☑"),
        ("- [X] ", "☑"),
        ("- [ ] ", "☐"),
        ("* [x] ", "☑"),
        ("* [X] ", "☑"),
        ("* [ ] ", "☐"),
        ("- ", "•"),
        ("* ", "•"),
    ] {
        if let Some(value) = line.strip_prefix(prefix) {
            return Some((marker.to_string(), value.trim()));
        }
    }

    let (number, value) = line.split_once(". ")?;
    if number.chars().all(|ch| ch.is_ascii_digit()) {
        Some((format!("{number}."), value.trim()))
    } else {
        None
    }
}

pub(crate) fn gherkin_line(line: &str) -> Option<(&str, &str)> {
    for keyword in [
        "Feature:",
        "Scenario:",
        "Scenario Outline:",
        "Given",
        "When",
        "Then",
        "And",
        "But",
        "Examples:",
    ] {
        if line == keyword {
            return Some((keyword, ""));
        }
        if let Some(rest) = line.strip_prefix(&format!("{keyword} ")) {
            return Some((keyword, rest.trim()));
        }
    }
    None
}

pub(crate) fn clean_inline_markdown(value: &str) -> String {
    value
        .replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .trim()
        .to_string()
}

pub(crate) fn is_markdown_table_line(line: &str) -> bool {
    line.starts_with('|') && line.matches('|').count() >= 2
}

pub(crate) fn flush_markdown_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    table_lines: &mut Vec<String>,
) {
    if table_lines.is_empty() {
        return;
    }
    push_markdown_table(output, theme, width, table_lines);
    table_lines.clear();
}

pub(crate) fn push_markdown_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    lines: &[String],
) {
    let rows = lines
        .iter()
        .filter(|line| !is_markdown_table_separator(line))
        .map(|line| parse_markdown_table_row(line))
        .filter(|cells| !cells.is_empty())
        .collect::<Vec<_>>();
    let Some((header, body)) = rows.split_first() else {
        return;
    };
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    if column_count == 0 {
        return;
    }

    let columns = markdown_table_columns(width, header, body, column_count);
    let body_rows = body
        .iter()
        .map(|row| {
            normalize_markdown_row(row, column_count)
                .into_iter()
                .map(TableCell::new)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    push_wrapped_table(output, theme, &columns, &body_rows);
}

pub(crate) fn parse_markdown_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| clean_inline_markdown(cell.trim()))
        .collect()
}

pub(crate) fn is_markdown_table_separator(line: &str) -> bool {
    line.trim()
        .trim_matches('|')
        .split('|')
        .all(|cell| cell.trim().chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

pub(crate) fn normalize_markdown_row(row: &[String], column_count: usize) -> Vec<String> {
    (0..column_count)
        .map(|index| row.get(index).cloned().unwrap_or_default())
        .collect()
}

pub(crate) fn markdown_table_columns(
    width: usize,
    header: &[String],
    body: &[Vec<String>],
    column_count: usize,
) -> Vec<DynamicTableColumn> {
    let available = row_content_width(width, column_count);
    let min_width = (available / column_count).clamp(1, 8);
    let mut widths = (0..column_count)
        .map(|index| {
            std::iter::once(header.get(index).map(String::as_str).unwrap_or(""))
                .chain(
                    body.iter()
                        .map(move |row| row.get(index).map(String::as_str).unwrap_or("")),
                )
                .map(display_width)
                .max()
                .unwrap_or(min_width)
                .max(min_width)
        })
        .collect::<Vec<_>>();

    while widths.iter().sum::<usize>() > available {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > min_width)
            .max_by_key(|(_, width)| **width)
        else {
            break;
        };
        widths[index] -= 1;
    }

    (0..column_count)
        .map(|index| DynamicTableColumn {
            title: header.get(index).cloned().unwrap_or_default(),
            width: widths.get(index).copied().unwrap_or(min_width),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenced_gherkin_blocks_are_syntax_highlighted() {
        let theme = Theme::color();
        let mut output = String::new();

        push_terminal_markdown(
            &mut output,
            &theme,
            100,
            "```gherkin\nGiven a developer opens a pull request\nWhen the pipeline runs\nThen the status is visible\n```",
        );

        assert!(output.contains("  │ "));
        assert!(output.contains("\x1b[1mGiven\x1b[0m a developer opens a pull request"));
        assert!(output.contains("\x1b[1mWhen\x1b[0m the pipeline runs"));
        assert!(output.contains("\x1b[1mThen\x1b[0m the status is visible"));
    }
}
