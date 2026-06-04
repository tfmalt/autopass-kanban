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
    push_terminal_markdown_indented(output, theme, width, content, 0);
}

pub(crate) fn push_terminal_markdown_indented(
    output: &mut String,
    theme: &Theme,
    width: usize,
    content: &str,
    indent: usize,
) {
    let normalized = normalize_terminal_markdown(content);
    let mut table_lines = Vec::new();
    let mut code_block = CodeBlockKind::None;

    for raw_line in normalized.lines() {
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
            push_code_block_line(output, theme, width, line, code_block, indent);
            continue;
        }

        push_terminal_markdown_line(output, theme, width, line, indent);
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
    indent: usize,
) {
    let prefix = " ".repeat(indent);
    match code_block {
        CodeBlockKind::Gherkin => push_gherkin_code_line(output, theme, width, line, indent),
        CodeBlockKind::Plain => {
            push_wrapped_hanging_line(output, &format!("{prefix}  │ "), line, width, |value| {
                theme.path(value)
            });
        }
        CodeBlockKind::None => {}
    }
}

pub(crate) fn push_gherkin_code_line(
    output: &mut String,
    theme: &Theme,
    width: usize,
    line: &str,
    indent: usize,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        push_line(output, &format!("{}  │", " ".repeat(indent)));
        return;
    }

    if let Some((keyword, rest)) = gherkin_line(trimmed) {
        push_wrapped_hanging_line(
            output,
            &format!("{}  │ ", " ".repeat(indent)),
            &format!("{} {}", theme.label(keyword), clean_inline_markdown(rest)),
            width,
            |value| value.to_string(),
        );
    } else {
        push_wrapped_hanging_line(
            output,
            &format!("{}  │ ", " ".repeat(indent)),
            trimmed,
            width,
            |value| theme.path(value),
        );
    }
}

pub(crate) fn push_terminal_markdown_line(
    output: &mut String,
    theme: &Theme,
    width: usize,
    line: &str,
    indent: usize,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        push_line(output, "");
        return;
    }

    let indent_prefix = " ".repeat(indent);

    if let Some(heading) = markdown_heading_text(trimmed) {
        push_wrapped_markdown_hanging_line(
            output,
            theme,
            &indent_prefix,
            heading,
            width,
            MarkdownLineKind::Heading,
        );
        return;
    }

    if let Some(quote) = trimmed.strip_prefix('>') {
        push_wrapped_markdown_hanging_line(
            output,
            theme,
            &format!("{indent_prefix}  │ "),
            quote.trim(),
            width,
            MarkdownLineKind::Body,
        );
        return;
    }

        if let Some((_indent, marker, value)) = markdown_list_item_with_indent(line) {
            push_wrapped_markdown_hanging_line(
                output,
                theme,
                &format!("{indent_prefix}{marker} "),
                value,
                width,
                MarkdownLineKind::Body,
            );
            return;
    }

    if let Some((keyword, rest)) = gherkin_line(trimmed) {
        push_wrapped_markdown_hanging_line(
            output,
            theme,
            &format!("{indent_prefix}{} ", theme.label(keyword)),
            rest,
            width,
            MarkdownLineKind::Body,
        );
        return;
    }

    push_wrapped_markdown_hanging_line(
        output,
        theme,
        &format!("{indent_prefix}  "),
        trimmed,
        width,
        MarkdownLineKind::Body,
    );
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum MarkdownInlineStyle {
    Plain,
    Strong,
    Code,
}

#[derive(Copy, Clone)]
enum MarkdownLineKind {
    Body,
    Heading,
}

struct MarkdownToken {
    text: String,
    style: MarkdownInlineStyle,
    leading_space: bool,
}

fn push_wrapped_markdown_hanging_line(
    output: &mut String,
    theme: &Theme,
    prefix: &str,
    value: &str,
    width: usize,
    line_kind: MarkdownLineKind,
) {
    let value_width = width.saturating_sub(display_width(prefix)).max(1);
    let continuation_prefix = " ".repeat(display_width(prefix));
    let mut rendered_lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for token in tokenize_markdown_inline(value) {
        let token_width = display_width(&token.text);
        let token_rendered = render_markdown_inline(theme, line_kind, token.style, &token.text);
        let needs_space = token.leading_space && !current.is_empty();

        if token_width > value_width {
            if !current.is_empty() {
                rendered_lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            for chunk in split_text_by_width(&token.text, value_width) {
                rendered_lines.push(render_markdown_inline(
                    theme,
                    line_kind,
                    token.style,
                    &chunk,
                ));
            }
            continue;
        }

        if !current.is_empty()
            && current_width + usize::from(needs_space) + token_width > value_width
        {
            rendered_lines.push(std::mem::take(&mut current));
            current_width = 0;
        }

        if token.leading_space && !current.is_empty() {
            current.push(' ');
            current_width += 1;
        }

        current.push_str(&token_rendered);
        current_width += token_width;
    }

    if !current.is_empty() {
        rendered_lines.push(current);
    }
    if rendered_lines.is_empty() {
        rendered_lines.push(String::new());
    }

    for (index, line) in rendered_lines.iter().enumerate() {
        if index == 0 {
            push_line(output, &format!("{prefix}{line}"));
        } else {
            push_line(output, &format!("{continuation_prefix}{line}"));
        }
    }
}

fn render_markdown_inline(
    theme: &Theme,
    line_kind: MarkdownLineKind,
    style: MarkdownInlineStyle,
    value: &str,
) -> String {
    match style {
        MarkdownInlineStyle::Strong => theme.highlight(value),
        MarkdownInlineStyle::Code => theme.path(value),
        MarkdownInlineStyle::Plain => match line_kind {
            MarkdownLineKind::Heading => theme.heading(value),
            MarkdownLineKind::Body => value.to_string(),
        },
    }
}

fn tokenize_markdown_inline(value: &str) -> Vec<MarkdownToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut style = MarkdownInlineStyle::Plain;
    let mut pending_space = false;
    let mut leading_space = false;
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if !current.is_empty() {
                    tokens.push(MarkdownToken {
                        text: std::mem::take(&mut current),
                        style,
                        leading_space,
                    });
                    leading_space = false;
                }
                style = toggle_inline_style(style, MarkdownInlineStyle::Strong);
            }
            '_' if chars.peek() == Some(&'_') => {
                chars.next();
                if !current.is_empty() {
                    tokens.push(MarkdownToken {
                        text: std::mem::take(&mut current),
                        style,
                        leading_space,
                    });
                    leading_space = false;
                }
                style = toggle_inline_style(style, MarkdownInlineStyle::Strong);
            }
            '`' => {
                if !current.is_empty() {
                    tokens.push(MarkdownToken {
                        text: std::mem::take(&mut current),
                        style,
                        leading_space,
                    });
                    leading_space = false;
                }
                style = toggle_inline_style(style, MarkdownInlineStyle::Code);
            }
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(MarkdownToken {
                        text: std::mem::take(&mut current),
                        style,
                        leading_space,
                    });
                    leading_space = false;
                }
                pending_space = true;
            }
            ch => {
                if current.is_empty() {
                    leading_space = pending_space;
                    pending_space = false;
                }
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(MarkdownToken {
            text: current,
            style,
            leading_space,
        });
    }

    tokens
}

fn toggle_inline_style(
    current: MarkdownInlineStyle,
    target: MarkdownInlineStyle,
) -> MarkdownInlineStyle {
    if current == target {
        MarkdownInlineStyle::Plain
    } else {
        target
    }
}

fn split_text_by_width(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();

    for ch in value.chars() {
        if display_width(&current) == width {
            lines.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn normalize_terminal_markdown(content: &str) -> String {
    let mut output = Vec::new();
    let lines = content.replace("\r\n", "\n");
    let lines: Vec<&str> = lines.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index].trim_end();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            output.push(String::new());
            index += 1;
            continue;
        }

        if trimmed.starts_with("```") {
            output.push(line.to_string());
            index += 1;
            while index < lines.len() {
                let code_line = lines[index].trim_end();
                output.push(code_line.to_string());
                index += 1;
                if code_line.trim().starts_with("```") {
                    break;
                }
            }
            continue;
        }

        if is_markdown_table_line(trimmed)
            || markdown_heading_text(trimmed).is_some()
            || trimmed.starts_with('>')
            || markdown_list_item(trimmed).is_some()
        {
            if let Some((indent, marker, value)) = markdown_list_item_with_indent(line) {
                let mut joined = value.to_string();
                index += 1;
                while index < lines.len() {
                    let next_line = lines[index].trim_end();
                    let next_trimmed = next_line.trim();
                    if next_trimmed.is_empty()
                        || next_trimmed.starts_with("```")
                        || is_markdown_table_line(next_trimmed)
                        || markdown_heading_text(next_trimmed).is_some()
                        || next_trimmed.starts_with('>')
                        || markdown_list_item_with_indent(next_line).is_some()
                    {
                        break;
                    }
                    joined.push(' ');
                    joined.push_str(next_trimmed);
                    index += 1;
                }
                output.push(format!("{}{} {}", " ".repeat(indent), marker, joined));
                continue;
            }

            output.push(line.to_string());
            index += 1;
            continue;
        }

        let mut paragraph = trimmed.to_string();
        index += 1;
        while index < lines.len() {
            let next_line = lines[index].trim_end();
            let next_trimmed = next_line.trim();
            if next_trimmed.is_empty()
                || next_trimmed.starts_with("```")
                || is_markdown_table_line(next_trimmed)
                || markdown_heading_text(next_trimmed).is_some()
                || next_trimmed.starts_with('>')
                || markdown_list_item(next_trimmed).is_some()
            {
                break;
            }
            paragraph.push(' ');
            paragraph.push_str(next_trimmed);
            index += 1;
        }
        output.push(paragraph);
    }

    output.join("\n")
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

fn markdown_list_item_with_indent(line: &str) -> Option<(usize, String, &str)> {
    let indent = line
        .find(|ch: char| !ch.is_whitespace())
        .unwrap_or(line.len());
    let trimmed = &line[indent..];
    let (marker, value) = markdown_list_item(trimmed)?;
    Some((indent, marker, value))
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

    #[test]
    fn markdown_blocks_keep_headings_lists_and_strong_text() {
        let theme = Theme::color();
        let mut output = String::new();

        push_terminal_markdown_indented(
            &mut output,
            &theme,
            80,
            "# Sprint Goal\n\nBuild **visible** value\n\n- First item\n- Second item",
            4,
        );

        assert!(output.contains("\x1b[1;36mSprint\x1b[0m \x1b[1;36mGoal\x1b[0m"));
        assert!(output.contains("\x1b[1;35mvisible\x1b[0m"));
        assert!(output.contains("• First item"));
        assert!(output.contains("• Second item"));
    }

    #[test]
    fn markdown_list_items_keep_hanging_indentation_when_wrapping() {
        let theme = Theme::plain();
        let mut output = String::new();

        push_terminal_markdown_indented(
            &mut output,
            &theme,
            200,
            "- One two three four five six seven",
            2,
        );

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines, vec!["    • One two three four five six seven"]);
    }

    #[test]
    fn markdown_collapses_soft_line_breaks_in_bullet_items() {
        let theme = Theme::plain();
        let mut output = String::new();

        push_terminal_markdown_indented(
            &mut output,
            &theme,
            200,
            "- Ha ferdig pipeline for CI/CD og utviklingsmiljø for backend\n  (java/quarkus) og frontend (web/react/vite).",
            2,
        );

        assert_eq!(output.lines().count(), 1);
        assert!(output.contains("backend (java/quarkus) og frontend (web/react/vite)."));
        assert!(output.starts_with("    • "));
    }
}
