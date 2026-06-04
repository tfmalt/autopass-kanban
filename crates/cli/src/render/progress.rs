#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn render_progress_bar(
    theme: &Theme,
    done: usize,
    in_progress: usize,
    total: usize,
    width: usize,
) -> String {
    let bar_width = (width / 5).clamp(8, 24).saturating_sub(2);
    let body_width = bar_width.saturating_sub(2);
    let total_units = body_width * 8;
    let total = total.max(1);
    let done = done.min(total);
    let active = done.saturating_add(in_progress).min(total);
    let done_units = scaled_bar_units(done, total, total_units);
    let active_units = scaled_bar_units(active, total, total_units);
    let mut bar = String::new();

    let first_segment = progress_segment_for_unit(0, done_units, active_units);
    bar.push_str(&theme.paint(progress_segment_style(first_segment), "\u{e0b6}"));

    for cell in 0..body_width {
        let start = cell * 8;
        let first = progress_segment_for_unit(start, done_units, active_units);
        let split = (1..8)
            .find(|offset| {
                progress_segment_for_unit(start + offset, done_units, active_units) != first
            })
            .unwrap_or(8);
        if split == 8 {
            bar.push_str(&theme.paint(
                progress_segment_style(first),
                progress_segment_full_char(first),
            ));
        } else {
            let next = progress_segment_for_unit(start + split, done_units, active_units);
            bar.push_str(&theme.paint_with_background(
                progress_segment_style(first),
                progress_segment_style(next),
                progress_fraction_char(split),
            ));
        }
    }

    let last_segment =
        progress_segment_for_unit(total_units.saturating_sub(1), done_units, active_units);
    bar.push_str(&theme.paint(progress_segment_style(last_segment), "\u{e0b4}"));

    bar
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) enum ProgressSegment {
    Done,
    InProgress,
    Empty,
}

pub(crate) fn scaled_bar_units(value: usize, total: usize, total_units: usize) -> usize {
    (value * total_units + total / 2) / total
}

pub(crate) fn progress_segment_for_unit(
    unit: usize,
    done_units: usize,
    active_units: usize,
) -> ProgressSegment {
    if unit < done_units {
        ProgressSegment::Done
    } else if unit < active_units {
        ProgressSegment::InProgress
    } else {
        ProgressSegment::Empty
    }
}

pub(crate) fn progress_segment_style(segment: ProgressSegment) -> Style {
    match segment {
        ProgressSegment::Done => Style::Green,
        ProgressSegment::InProgress => Style::Blue,
        ProgressSegment::Empty => Style::DarkGray,
    }
}

pub(crate) fn progress_segment_full_char(segment: ProgressSegment) -> &'static str {
    match segment {
        ProgressSegment::Done | ProgressSegment::InProgress => "█",
        ProgressSegment::Empty => "░",
    }
}

pub(crate) fn progress_fraction_char(units: usize) -> &'static str {
    match units {
        1 => "▏",
        2 => "▎",
        3 => "▍",
        4 => "▌",
        5 => "▋",
        6 => "▊",
        7 => "▉",
        _ => "█",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_bar_scales_with_terminal_width() {
        let theme = Theme::plain();
        let bar_80 = render_progress_bar(&theme, 6, 4, 14, 80);
        let bar_120 = render_progress_bar(&theme, 6, 4, 14, 120);
        assert_eq!(display_width(&bar_80), 80 / 5 - 2);
        assert_eq!(display_width(&bar_120), 120 / 5 - 2);
        assert!(bar_80.starts_with("\u{e0b6}"));
        assert!(bar_80.ends_with("\u{e0b4}"));
    }

    #[test]
    fn progress_bar_uses_done_and_in_progress_status_colors() {
        let theme = Theme::color();
        let bar = render_progress_bar(&theme, 5, 3, 10, 100);

        assert!(
            bar.contains("\x1b[1;32m"),
            "done segment should be green: {bar}"
        );
        assert!(
            bar.contains("\x1b[1;34m"),
            "in-progress segment should be blue: {bar}"
        );
        assert!(
            bar.contains("\x1b[1;34;40m"),
            "in-progress boundary should use dark gray background: {bar}"
        );
        assert!(
            bar.contains("\x1b[90m\u{e0b4}"),
            "right cap should use dark gray foreground: {bar}"
        );
        assert_eq!(display_width(&bar), 100 / 5 - 2);
    }

    #[test]
    fn progress_bar_uses_eighth_block_resolution() {
        let plain = render_progress_bar(&Theme::plain(), 1, 0, 7, 100);
        assert!(
            plain.contains("▎"),
            "expected one-quarter boundary after cap columns: {plain}"
        );

        let colored = render_progress_bar(&Theme::color(), 1, 1, 7, 100);
        assert!(
            colored.contains("\x1b[1;32;44m▎"),
            "done to in-progress boundary should use green foreground and blue background: {colored}"
        );
    }
}
