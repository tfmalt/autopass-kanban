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
pub(crate) enum InlineStyle {
    Plain,
    Command,
}

pub(crate) struct InlineToken {
    pub(crate) text: &'static str,
    pub(crate) style: InlineStyle,
    pub(crate) leading_space: bool,
}

impl InlineToken {
    pub(crate) const fn plain(text: &'static str, leading_space: bool) -> Self {
        Self {
            text,
            style: InlineStyle::Plain,
            leading_space,
        }
    }

    pub(crate) const fn command(text: &'static str, leading_space: bool) -> Self {
        Self {
            text,
            style: InlineStyle::Command,
            leading_space,
        }
    }
}

pub(crate) fn push_wrapped_inline_message(
    output: &mut String,
    theme: &Theme,
    indent: usize,
    width: usize,
    tokens: &[InlineToken],
) {
    let mut lines: Vec<Vec<(InlineStyle, String)>> = Vec::new();
    let mut current: Vec<(InlineStyle, String)> = Vec::new();
    let mut current_width = 0;

    for token in tokens {
        let token_width = display_width(token.text);
        let space_width = usize::from(token.leading_space && !current.is_empty());
        if !current.is_empty() && current_width + space_width + token_width > width {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }

        if token.leading_space && !current.is_empty() {
            current.push((InlineStyle::Plain, " ".to_string()));
            current_width += 1;
        }
        current.push((token.style, token.text.to_string()));
        current_width += token_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    for line in lines {
        let mut rendered = " ".repeat(indent);
        for (style, text) in line {
            match style {
                InlineStyle::Plain => rendered.push_str(&text),
                InlineStyle::Command => rendered.push_str(&theme.command(text)),
            }
        }
        push_line(output, &rendered);
    }
}
