use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use kanban_core::*;

use crate::dto::*;
use crate::metrics::{DashboardMetrics, compute_metrics};
use crate::snapshot::{
    build_epics, build_sprints, compute_progress, epic_body_index, web_story_from_core,
};

/// Everything the web read endpoints need, derived from **one** repository
/// read.
///
/// Before this type existed, `/api/repository` performed roughly `2 + S + E *
/// (S + 3)` configuration loads (each one a `git rev-parse --show-toplevel`
/// subprocess plus a `settings.json` parse) because `load_epics` called
/// `find_epic` — a full repository read — once per epic. `/api/metrics` and
/// `/api/report` then repeated the whole thing.
///
/// The invariant this type exists to hold: **repository, progress, metrics and
/// report data served together derive from the same source read**, so a client
/// can never observe two endpoints disagreeing about the same generation of the
/// markdown backlog.
pub(crate) struct WebReadModel {
    pub(crate) snapshot: RepositorySnapshot,
    /// Deduplicated core story overviews (`list_all_stories` equivalent).
    pub(crate) story_overviews: Vec<StoryOverview>,
    /// Core sprint overviews (`summarize_sprints` equivalent).
    pub(crate) sprint_overviews: Vec<SprintOverview>,
    /// Epic body markdown keyed by uppercased epic id, for `/api/epics/{id}`.
    epic_bodies: BTreeMap<String, String>,
}

impl WebReadModel {
    /// Build every projection from a single repository read.
    pub(crate) fn build(repo_root: &Path) -> Result<Self> {
        let config = load_kanban_config(repo_root)?;
        let repository = read_repository_with_config(&config)?;

        let mut stories = repository
            .stories
            .iter()
            .map(|story| web_story_from_core(&repository.repo_root, story))
            .collect::<Vec<_>>();
        stories.sort_by(|a, b| a.id.cmp(&b.id));

        let epic_sources = read_epic_sources(&config)?;
        let epics = build_epics(&epic_sources, &stories);
        let epic_bodies = epic_body_index(&epic_sources);

        let sprint_overviews = summarize_sprints_from_repository(&repository, &config)?;
        let sprints = build_sprints(&sprint_overviews, &stories);
        let progress = compute_progress(&stories);
        let story_overviews = story_overviews_from_repository(&repository);

        Ok(Self {
            snapshot: RepositorySnapshot {
                stories,
                epics,
                sprints,
                progress,
            },
            story_overviews,
            sprint_overviews,
            epic_bodies,
        })
    }

    pub(crate) fn into_snapshot(self) -> RepositorySnapshot {
        self.snapshot
    }

    pub(crate) fn metrics(&self) -> DashboardMetrics {
        compute_metrics(
            &self.snapshot,
            &self.story_overviews,
            &self.sprint_overviews,
        )
    }

    pub(crate) fn report(&self) -> WebReportDashboard {
        let current_sprint_name = self
            .sprint_overviews
            .iter()
            .find(|sprint| sprint.readme_status.as_deref() == Some("active"))
            .map(|sprint| sprint.sprint_name.as_str());
        WebReportDashboard::from(ReportDashboardDto::build(
            &self.story_overviews,
            &self.sprint_overviews,
            current_sprint_name,
        ))
    }

    /// Epic detail for `/api/epics/{id}`: the epic with its child stories sorted
    /// by id, plus the epic body markdown.
    ///
    /// The body falls back to an empty string when no epic file carries the id
    /// (the epic exists only because stories reference it), matching the
    /// previous `find_epic_with_source` behavior.
    pub(crate) fn epic_detail(mut self, id: &str) -> Option<(WebEpic, String)> {
        let index = self
            .snapshot
            .epics
            .iter()
            .position(|epic| epic.id.eq_ignore_ascii_case(id))?;
        let mut epic = self.snapshot.epics.swap_remove(index);
        epic.stories.sort_by(|a, b| a.id.cmp(&b.id));
        let body = self
            .epic_bodies
            .remove(&id.trim().to_ascii_uppercase())
            .unwrap_or_default();
        Some((epic, body))
    }
}

#[cfg(test)]
mod tests;
