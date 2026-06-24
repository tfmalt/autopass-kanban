#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

/// The resolved scope for a `kanban story list` invocation (US-020).
///
/// Both the human and JSON output paths call [`resolve_story_list_scope`] to
/// get this value plus the story list, then format the scope label their own
/// way. This ensures scope resolution logic exists in exactly one place.
#[derive(Debug, Clone)]
pub(crate) enum StoryListScope {
    All,
    Next { sprint_name: String },
    Sprint { sprint_name: String },
    Current { sprint_name: String },
    Active { sprint_name: String },
}

impl StoryListScope {
    /// Short machine-readable label used by the JSON `StoryListDto.scope` field.
    pub(crate) fn json_label(&self) -> String {
        match self {
            StoryListScope::All => "all".to_string(),
            StoryListScope::Next { .. } => "next".to_string(),
            StoryListScope::Sprint { sprint_name } => format!("sprint:{sprint_name}"),
            // Both Current and Active map to "current" in JSON for backward
            // compatibility — the old JSON path didn't distinguish them.
            StoryListScope::Current { .. } | StoryListScope::Active { .. } => "current".to_string(),
        }
    }

    /// Human-readable label used by the terminal `print_story_list` output.
    pub(crate) fn human_label(&self) -> String {
        match self {
            StoryListScope::All => "all stories".to_string(),
            StoryListScope::Next { sprint_name } => format!("next sprint ({sprint_name})"),
            StoryListScope::Sprint { sprint_name } => format!("sprint {sprint_name}"),
            StoryListScope::Current { sprint_name } => {
                format!("current sprint ({sprint_name})")
            }
            StoryListScope::Active { sprint_name } => {
                format!("active sprint ({sprint_name})")
            }
        }
    }
}

/// Resolve the story-list scope from CLI flags, shared by both the human and
/// JSON output paths (US-020). Returns the structured scope and the matching
/// story list.
///
/// When sprints are disabled, `--next` and `--sprint` produce an error; the
/// default (no flags) falls back to all stories.
pub(crate) fn resolve_story_list_scope(
    repo_root: &Path,
    all: bool,
    next: bool,
    current: bool,
    sprint: Option<&str>,
) -> Result<(StoryListScope, Vec<StoryOverview>)> {
    if all {
        let stories = list_all_stories(repo_root)?;
        return Ok((StoryListScope::All, stories));
    }

    let config = kanban_core::load_kanban_config(repo_root)?;
    let sprints_enabled = config.features().sprints;

    if next {
        if !sprints_enabled {
            bail!(
                "Feature 'sprints' is disabled in .kanban/settings.json. Run `kanban features enable sprints` to re-enable it. (repo: {})",
                repo_root.display()
            );
        }
        let (sprint_name, stories) = list_next_sprint_stories(repo_root)?;
        return Ok((StoryListScope::Next { sprint_name }, stories));
    }

    if let Some(sprint_name) = sprint {
        if !sprints_enabled {
            bail!(
                "Feature 'sprints' is disabled in .kanban/settings.json. Run `kanban features enable sprints` to re-enable it. (repo: {})",
                repo_root.display()
            );
        }
        let stories = list_stories_in_sprint(repo_root, sprint_name)?;
        return Ok((
            StoryListScope::Sprint {
                sprint_name: sprint_name.to_string(),
            },
            stories,
        ));
    }

    if !sprints_enabled {
        let stories = list_all_stories(repo_root)?;
        return Ok((StoryListScope::All, stories));
    }

    let (sprint_name, stories) = list_current_sprint_stories(repo_root)?;
    let scope = if current {
        StoryListScope::Current { sprint_name }
    } else {
        StoryListScope::Active { sprint_name }
    };
    Ok((scope, stories))
}

/// Build a `CreateSprintInput` from CLI flags and suggested defaults, shared
/// by both the human and JSON output paths (US-020).
///
/// Requires `headline` (the caller validates this before calling). When
/// `--number`/`--start`/`--end` are omitted, suggested defaults are derived
/// from the repository's sprint history.
pub(crate) fn build_create_sprint_input_from_flags(
    repo_root: &Path,
    number: Option<u32>,
    headline: &str,
    start: Option<&str>,
    end: Option<&str>,
) -> Result<CreateSprintInput> {
    let (suggested_number, repo_suggestion) = suggested_sprint_defaults(&repo_root.to_path_buf())?;
    let number = number.unwrap_or(suggested_number);
    let today = chrono::Local::now().date_naive();
    let start_date = match start {
        Some(value) => NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
            .map_err(|_| anyhow::anyhow!("--start must be a date as YYYY-MM-DD."))?,
        None => repo_suggestion
            .map(|(start_date, _)| start_date)
            .unwrap_or(today),
    };
    let end_date = match end {
        Some(value) => NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
            .map_err(|_| anyhow::anyhow!("--end must be a date as YYYY-MM-DD."))?,
        None => repo_suggestion
            .map(|(_, end_date)| end_date)
            .unwrap_or_else(|| suggested_sprint_dates(start_date).1),
    };
    Ok(CreateSprintInput {
        number,
        start_date,
        end_date,
        headline: headline.to_string(),
    })
}
