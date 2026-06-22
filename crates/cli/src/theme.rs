#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

#[derive(Copy, Clone)]
pub(crate) struct Theme {
    pub(crate) color: bool,
}

#[derive(Copy, Clone)]
pub(crate) enum Style {
    Bold,
    DarkGray,
    Muted,
    Amber,
    Blue,
    Cyan,
    Green,
    Purple,
    Red,
    Yellow,
}

impl Theme {
    pub(crate) fn for_stdout(color_mode: ColorMode) -> Self {
        Self {
            color: match color_mode {
                ColorMode::Always => true,
                ColorMode::Never => false,
                ColorMode::Auto => {
                    std::io::stdout().is_terminal()
                        && std::env::var_os("NO_COLOR").is_none()
                        && std::env::var_os("TERM").is_none_or(|term| term != "dumb")
                }
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn color() -> Self {
        Self { color: true }
    }

    #[cfg(test)]
    pub(crate) fn plain() -> Self {
        Self { color: false }
    }

    pub(crate) fn paint(&self, style: Style, value: impl std::fmt::Display) -> String {
        if !self.color {
            return value.to_string();
        }

        let code = foreground_code(style);
        format!("\x1b[{code}m{value}\x1b[0m")
    }

    pub(crate) fn paint_with_background(
        &self,
        foreground: Style,
        background: Style,
        value: impl std::fmt::Display,
    ) -> String {
        if !self.color {
            return value.to_string();
        }

        format!(
            "\x1b[{};{}m{value}\x1b[0m",
            foreground_code(foreground),
            background_code(background)
        )
    }

    pub(crate) fn heading(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Cyan, value)
    }

    pub(crate) fn label(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Bold, value)
    }

    pub(crate) fn id(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Cyan, value)
    }

    pub(crate) fn count(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Bold, value)
    }

    pub(crate) fn story_points(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Yellow, value)
    }

    pub(crate) fn path(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Muted, value)
    }

    pub(crate) fn success(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Green, value)
    }

    pub(crate) fn warning(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Yellow, value)
    }

    pub(crate) fn error(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Red, value)
    }

    pub(crate) fn command(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Blue, value)
    }

    pub(crate) fn highlight(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Purple, value)
    }

    pub(crate) fn brand(&self) -> String {
        self.paint(Style::Amber, "kanban")
    }

    pub(crate) fn version(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Green, value)
    }

    pub(crate) fn status(&self, status: &str) -> String {
        match status {
            "backlog" | "ready" => self.paint(Style::Muted, status),
            "todo" => self.paint(Style::Muted, status),
            "in-progress" => self.paint(Style::Blue, status),
            "ready-for-qa" => self.paint(Style::Purple, status),
            "done" => self.paint(Style::Green, status),
            "blocked" => self.paint(Style::Red, status),
            _ => status.to_string(),
        }
    }

    pub(crate) fn status_text(&self, status: &str, text: impl std::fmt::Display) -> String {
        match status {
            "backlog" | "ready" => self.paint(Style::Muted, text),
            "todo" => self.paint(Style::Muted, text),
            "in-progress" => self.paint(Style::Blue, text),
            "ready-for-qa" => self.paint(Style::Purple, text),
            "done" => self.paint(Style::Green, text),
            "blocked" => self.paint(Style::Red, text),
            _ => text.to_string(),
        }
    }

    pub(crate) fn severity(&self, severity: &str) -> String {
        match severity.to_ascii_lowercase().as_str() {
            "error" | "critical" => self.paint(Style::Red, severity),
            "warning" | "warn" => self.paint(Style::Yellow, severity),
            "info" => self.paint(Style::Cyan, severity),
            _ => severity.to_string(),
        }
    }
}

pub(crate) fn foreground_code(style: Style) -> &'static str {
    match style {
        Style::Bold => "1",
        Style::DarkGray => "90",
        Style::Muted => "2",
        Style::Amber => "93",
        Style::Blue => "1;34",
        Style::Cyan => "1;36",
        Style::Green => "1;32",
        Style::Purple => "1;35",
        Style::Red => "1;31",
        Style::Yellow => "1;33",
    }
}

pub(crate) fn background_code(style: Style) -> &'static str {
    match style {
        Style::Bold | Style::Muted => "100",
        Style::DarkGray => "40",
        Style::Amber => "43",
        Style::Blue => "44",
        Style::Cyan => "46",
        Style::Green => "42",
        Style::Purple => "45",
        Style::Red => "41",
        Style::Yellow => "43",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_theme_preserves_text_without_ansi_codes() {
        let theme = Theme::plain();

        assert_eq!(theme.status("blocked"), "blocked");
        assert_eq!(theme.id("US-F1-056"), "US-F1-056");
        assert!(!theme.status("done").contains("\x1b["));
    }

    #[test]
    fn color_theme_keeps_status_text_while_adding_ansi_codes() {
        let theme = Theme::color();
        let styled = theme.status("in-progress");

        assert!(styled.contains("\x1b["));
        assert!(styled.contains("in-progress"));
    }

    #[test]
    fn brand_and_version_use_distinct_colors() {
        let theme = Theme::color();

        assert_eq!(theme.brand(), "\x1b[93mkanban\x1b[0m");
        assert_eq!(theme.version("1.2.3"), "\x1b[1;32m1.2.3\x1b[0m");
    }
}
