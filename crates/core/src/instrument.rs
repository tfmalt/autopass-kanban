//! Test-only instrumentation counters for the repository read path.
//!
//! These counters exist so the budgets in the board/dashboard loading
//! improvement plan (B1-B4) can be asserted deterministically instead of being
//! inferred from wall-clock timings. They record how often the *expensive*
//! parts of a repository read happen:
//!
//! - `git rev-parse --show-toplevel` subprocess spawns (`git_root_resolutions`)
//! - `.kanban/settings.json` reads + parses (`settings_parses`)
//! - complete story markdown parses (`story_parses`)
//! - complete epic markdown parses (`epic_parses`)
//!
//! Counters are **thread-local**. Test binaries run tests concurrently, and a
//! process-global counter would be corrupted by unrelated tests building their
//! own fixtures. Every counted read path is synchronous on the calling thread,
//! so a thread-local count is exact and needs no locking.
//!
//! The module is only compiled when `kanban-core` is built for tests or with
//! the `test-support` feature. In a production build the recording functions
//! are inlined no-ops (see `lib.rs`), so no global mutable state ships.

use std::cell::Cell;

thread_local! {
    static GIT_ROOT_RESOLUTIONS: Cell<usize> = const { Cell::new(0) };
    static SETTINGS_PARSES: Cell<usize> = const { Cell::new(0) };
    static STORY_PARSES: Cell<usize> = const { Cell::new(0) };
    static EPIC_PARSES: Cell<usize> = const { Cell::new(0) };
}

#[inline]
fn bump(counter: &'static std::thread::LocalKey<Cell<usize>>) {
    // `try_with` so instrumentation can never panic during thread teardown.
    let _ = counter.try_with(|value| value.set(value.get() + 1));
}

#[inline]
pub fn record_git_root_resolution() {
    bump(&GIT_ROOT_RESOLUTIONS);
}

#[inline]
pub fn record_settings_parse() {
    bump(&SETTINGS_PARSES);
}

#[inline]
pub fn record_story_parse() {
    bump(&STORY_PARSES);
}

#[inline]
pub fn record_epic_parse() {
    bump(&EPIC_PARSES);
}

/// Handle over the calling thread's read-path counters.
///
/// Construct it immediately before the operation under test — construction
/// zeroes every counter — then read the counts afterwards.
#[must_use = "constructing the handle resets the counters; read them afterwards"]
#[derive(Debug)]
pub struct ReadPathCounters {
    _private: (),
}

impl Default for ReadPathCounters {
    fn default() -> Self {
        Self::start()
    }
}

impl ReadPathCounters {
    /// Zero every counter for the calling thread.
    pub fn start() -> Self {
        let counters = Self { _private: () };
        counters.reset();
        counters
    }

    pub fn reset(&self) {
        for counter in [
            &GIT_ROOT_RESOLUTIONS,
            &SETTINGS_PARSES,
            &STORY_PARSES,
            &EPIC_PARSES,
        ] {
            let _ = counter.try_with(|value| value.set(0));
        }
    }

    /// `git rev-parse --show-toplevel` subprocess spawns since the last reset.
    pub fn git_root_resolutions(&self) -> usize {
        GIT_ROOT_RESOLUTIONS.with(Cell::get)
    }

    /// `.kanban/settings.json` reads + deserializations since the last reset.
    pub fn settings_parses(&self) -> usize {
        SETTINGS_PARSES.with(Cell::get)
    }

    /// Complete story markdown parses since the last reset.
    pub fn story_parses(&self) -> usize {
        STORY_PARSES.with(Cell::get)
    }

    /// Complete epic markdown parses since the last reset.
    pub fn epic_parses(&self) -> usize {
        EPIC_PARSES.with(Cell::get)
    }

    pub fn snapshot(&self) -> ReadPathCounts {
        ReadPathCounts {
            git_root_resolutions: self.git_root_resolutions(),
            settings_parses: self.settings_parses(),
            story_parses: self.story_parses(),
            epic_parses: self.epic_parses(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadPathCounts {
    pub git_root_resolutions: usize,
    pub settings_parses: usize,
    pub story_parses: usize,
    pub epic_parses: usize,
}
