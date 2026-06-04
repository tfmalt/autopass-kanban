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
