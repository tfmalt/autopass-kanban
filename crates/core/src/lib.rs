mod availability;
mod config;
mod constants;
mod doctor;
mod epic;
mod error;
mod json;
mod lock;
mod markdown;
mod model;
mod phase;
mod regexes;
mod repository;
mod sprint;
mod sprint_roster;
mod status;
mod story;
#[cfg(any(test, feature = "test-support"))]
pub mod testsupport;
#[cfg(test)]
mod testutil;
mod user_config;
mod util;
mod validate;

/// Read-path instrumentation counters (B1-B4 in the loading improvement plan).
///
/// Only compiled for tests or with the `test-support` feature; production
/// builds get the inlined no-op shims below, so no global mutable state ships.
#[cfg(any(test, feature = "test-support"))]
pub mod instrument;

#[cfg(not(any(test, feature = "test-support")))]
mod instrument {
    #[inline(always)]
    pub(crate) fn record_git_root_resolution() {}
    #[inline(always)]
    pub(crate) fn record_settings_parse() {}
    #[inline(always)]
    pub(crate) fn record_story_parse() {}
    #[inline(always)]
    pub(crate) fn record_epic_parse() {}
}

pub(crate) mod prelude {
    pub(crate) use anyhow::{Context, Result, anyhow, bail};
    pub(crate) use chrono::{Datelike, Days, Local, NaiveDate, TimeZone, Weekday};
    #[allow(unused_imports)]
    pub(crate) use regex::Regex;
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    pub(crate) use std::fs;
    pub(crate) use std::path::{Path, PathBuf};
    pub(crate) use std::process::Command;
    pub(crate) use walkdir::WalkDir;
}

pub use availability::*;
pub use config::{
    ColorMode, ConfigInitResult, ConfigSetResult, FeaturesConfig, KanbanConfig, TeamMemberConfig,
    get_config_json, get_config_value, init_config, init_config_with_features, load_kanban_config,
    require_git_repo_root, resolve_repo_root, set_config_value,
};
pub use constants::*;
pub use doctor::*;
pub use epic::*;
pub use error::*;
pub use json::*;
pub use lock::*;
pub use markdown::*;
pub use model::*;
pub use phase::*;
pub use repository::*;
pub use sprint::*;
pub use status::*;
pub use story::*;
pub use user_config::{
    DEFAULT_REPO_ROOT_MARKER, GlobalConfigStatus, KANBAN_CONFIG_HOME, KANBAN_REPO_ROOT,
    clear_default_repo_root, effective_repo_root, global_config_status, set_default_repo_root,
    user_config_path,
};
pub use util::parse_assignee_list;
pub use validate::*;
