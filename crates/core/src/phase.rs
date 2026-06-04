use crate::config::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::story::*;
use crate::util::*;

pub fn summarize_phase(repo_root: impl AsRef<Path>, phase: &str) -> Result<PhaseOverview> {
    let repository = read_repository(repo_root)?;
    let phase_number = normalize_phase_input(phase)?;
    let config = load_kanban_config(&repository.repo_root)?;
    let phase_marker = format!("{}phase-{phase_number}-", config.backlog_marker());
    let mut stories = repository
        .stories
        .iter()
        .filter(|story| to_forward_slashes(&story.file_path).contains(&phase_marker))
        .map(|story| story_overview(&repository.repo_root, story))
        .collect::<Vec<_>>();

    stories.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(PhaseOverview {
        phase: format!("F{phase_number}"),
        stories,
    })
}

pub(crate) fn normalize_phase_input(phase: &str) -> Result<String> {
    let digits = phase
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(anyhow!(
            "Phase must contain a numeric identifier, for example `1` or `F1`."
        ));
    }

    let trimmed = digits.trim_start_matches('0');
    Ok(if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    })
}
