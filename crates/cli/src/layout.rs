#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, prompt::*, render::*, theme::*, web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) const MIN_TERMINAL_WIDTH: usize = 80;
pub(crate) const DEFAULT_OUTPUT_WIDTH: usize = 100;
pub(crate) const SPRINT_CONTENT_INSET: usize = 2;
pub(crate) const SPRINT_STORY_ROW_PREFIX: &str = "    · ";
#[derive(Copy, Clone)]
pub(crate) struct OutputLayout {
    pub(crate) width: usize,
}

impl OutputLayout {
    pub(crate) fn for_stdout() -> Result<Self> {
        let width = detected_terminal_width().unwrap_or(DEFAULT_OUTPUT_WIDTH);
        if width < MIN_TERMINAL_WIDTH {
            bail!(
                "Terminal width must be at least {MIN_TERMINAL_WIDTH} columns for kanban output; detected {width}."
            );
        }
        Ok(Self { width })
    }
}

pub(crate) fn detected_terminal_width() -> Option<usize> {
    if std::io::stdout().is_terminal() {
        terminal_width_from_stdout().or_else(terminal_width_from_columns)
    } else {
        terminal_width_from_columns()
    }
}

pub(crate) fn terminal_width_from_columns() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > 0)
}

#[cfg(unix)]
pub(crate) fn terminal_width_from_stdout() -> Option<usize> {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::zeroed();
    let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, size.as_mut_ptr()) };
    if result != 0 {
        return None;
    }

    let size = unsafe { size.assume_init() };
    (size.ws_col > 0).then_some(size.ws_col as usize)
}

#[cfg(not(unix))]
pub(crate) fn terminal_width_from_stdout() -> Option<usize> {
    None
}
