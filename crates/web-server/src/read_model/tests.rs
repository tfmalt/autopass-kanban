//! WP-03 guards: the read model must satisfy the read-path budgets (B2, B3) and
//! stay byte-equivalent to the projections it replaced.

use std::collections::BTreeMap;

use kanban_core::instrument::ReadPathCounters;
use kanban_core::testsupport::{BacklogFixture, FixtureSpec, generate_backlog_fixture};
use kanban_core::*;

use super::WebReadModel;
use crate::dto::{BOARD_STATUSES, WebEpic, WebStory};
use crate::snapshot::web_story_from_core;

/// `generatedAt` is `Local::now()`, so two derivations of the same data
/// legitimately differ by microseconds. Strip it everywhere before comparing.
fn without_generated_at(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .filter(|(key, _)| key != "generatedAt")
                .map(|(key, value)| (key, without_generated_at(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(without_generated_at).collect())
        }
        other => other,
    }
}

fn fixtures() -> Vec<BacklogFixture> {
    vec![
        generate_backlog_fixture(&FixtureSpec::representative().with_stories(60)),
        generate_backlog_fixture(&FixtureSpec::minimal()),
    ]
}

/// Reference implementation of the epic projection using the pre-WP-03
/// algorithm: read every epic file, resolve it through `find_epic`, and take
/// `details.epic`. It is quadratic, which is exactly why production no longer
/// does this — but it pins the expected output.
fn legacy_epics(repo_root: &std::path::Path, stories: &[WebStory]) -> Vec<WebEpic> {
    let mut epics = BTreeMap::<String, WebEpic>::new();
    for path in collect_epic_files(repo_root).unwrap() {
        let source = read_epic_file(&path, repo_root).unwrap();
        let source_overview = epic_overview(&source);
        let Some(details) = find_epic(repo_root, &source_overview.id).unwrap() else {
            continue;
        };
        let overview = details.epic;
        let id = overview.id.clone();
        epics.insert(
            id.clone(),
            WebEpic {
                title: overview.title,
                phase: overview.phase.unwrap_or_else(|| "F?".to_string()),
                priority: overview.priority,
                planned_start: overview.planned_start,
                planned_end: overview.planned_end,
                work_started: overview.work_started,
                work_done: overview.work_done,
                stories: Vec::new(),
                id,
            },
        );
    }
    for story in stories {
        if let Some(epic_id) = &story.epic {
            let entry = epics.entry(epic_id.clone()).or_insert_with(|| WebEpic {
                id: epic_id.clone(),
                title: epic_id.clone(),
                phase: phase_from_id(epic_id, "EP")
                    .unwrap_or_else(|| story.phase.clone().unwrap_or_else(|| "F?".to_string())),
                priority: None,
                planned_start: None,
                planned_end: None,
                work_started: None,
                work_done: None,
                stories: Vec::new(),
            });
            entry.stories.push(story.clone());
        }
    }
    epics.into_values().collect()
}

fn legacy_stories(repo_root: &std::path::Path) -> Vec<WebStory> {
    let repository = read_repository(repo_root).unwrap();
    let mut stories = repository
        .stories
        .iter()
        .map(|story| web_story_from_core(&repository.repo_root, story))
        .collect::<Vec<_>>();
    stories.sort_by(|a, b| a.id.cmp(&b.id));
    stories
}

/// B2 + B3: one web read-model build performs exactly one git root resolution,
/// one settings parse, one parse per story and one parse per epic file.
#[test]
fn read_model_build_reads_the_source_exactly_once() {
    for fixture in fixtures() {
        let story_files = collect_user_story_files(fixture.root()).unwrap().len();
        let epic_files = collect_epic_files(fixture.root()).unwrap().len();

        let counters = ReadPathCounters::start();
        let model = WebReadModel::build(fixture.root()).unwrap();
        let counts = counters.snapshot();

        assert_eq!(
            counts.git_root_resolutions, 1,
            "B2: one git root resolution per read-model build, got {counts:?}"
        );
        assert_eq!(
            counts.settings_parses, 1,
            "B4: one settings.json parse per read-model build, got {counts:?}"
        );
        assert_eq!(
            counts.story_parses, story_files,
            "B3: one complete story parse per story, got {counts:?}"
        );
        assert_eq!(
            counts.epic_parses, epic_files,
            "one epic parse per epic file, got {counts:?}"
        );

        // Every derived projection must be free: no further filesystem or git work.
        let counters = ReadPathCounters::start();
        let _metrics = model.metrics();
        let _report = model.report();
        let _detail = model.epic_detail("EP-001");
        let derived = counters.snapshot();
        drop(counters);
        assert_eq!(
            derived,
            kanban_core::instrument::ReadPathCounts {
                git_root_resolutions: 0,
                settings_parses: 0,
                story_parses: 0,
                epic_parses: 0,
            },
            "metrics, report and epic detail must derive from the built model"
        );
    }
}

/// R2 guard: removing `find_epic` from the epic projection must not change the
/// epic set, their metadata, or their child-story lists.
#[test]
fn epic_projection_matches_the_find_epic_based_algorithm() {
    for fixture in fixtures() {
        let expected_stories = legacy_stories(fixture.root());
        let expected = legacy_epics(fixture.root(), &expected_stories);
        let actual = WebReadModel::build(fixture.root()).unwrap().into_snapshot();

        assert_eq!(
            serde_json::to_value(&expected).unwrap(),
            serde_json::to_value(&actual.epics).unwrap(),
            "epic projection changed for {:?}",
            fixture.spec()
        );
        assert_eq!(
            serde_json::to_value(&expected_stories).unwrap(),
            serde_json::to_value(&actual.stories).unwrap(),
            "story projection changed for {:?}",
            fixture.spec()
        );
    }
}

/// A story whose `epic` names a file that does not exist keeps the synthesized
/// fallback epic rather than disappearing from the board.
#[test]
fn story_with_absent_epic_file_keeps_the_fallback_epic() {
    let fixture = generate_backlog_fixture(&FixtureSpec::representative().with_stories(60));
    let snapshot = WebReadModel::build(fixture.root()).unwrap().into_snapshot();

    let fallback = snapshot
        .epics
        .iter()
        .find(|epic| epic.id == "EP-999")
        .expect("fixture story US-002 references EP-999, which has no epic file");
    assert_eq!(fallback.title, "EP-999", "fallback epic titles use the id");
    assert!(
        fallback.stories.iter().any(|story| story.id == "US-002"),
        "the referencing story must still be grouped under the fallback epic"
    );

    // The story with no epic at all must not create an epic bucket.
    assert!(
        snapshot
            .epics
            .iter()
            .all(|epic| !epic.stories.iter().any(|story| story.id == "US-001")),
        "a story without an epic must not be grouped"
    );
}

/// Status aliases must land in the same board bucket as their canonical form,
/// and `dropped` must fold into `done`.
#[test]
fn status_aliases_bucket_into_the_same_board_columns() {
    let fixture = generate_backlog_fixture(&FixtureSpec::representative().with_stories(60));
    let snapshot = WebReadModel::build(fixture.root()).unwrap().into_snapshot();

    let sprint = snapshot
        .sprints
        .first()
        .expect("representative fixture has sprints");
    for status in BOARD_STATUSES {
        assert!(
            sprint.stories_by_status.contains_key(status),
            "every board column must exist, missing {status}"
        );
    }

    let aliased: Vec<&str> = snapshot
        .sprints
        .iter()
        .flat_map(|sprint| sprint.stories_by_status.iter())
        .filter(|(_, stories)| stories.iter().any(|story| story.status == "In Progress"))
        .map(|(status, _)| status.as_str())
        .collect();
    assert!(
        aliased.iter().all(|status| *status == "in-progress"),
        "the `In Progress` alias must bucket as in-progress, got {aliased:?}"
    );

    let dropped_in_done = snapshot
        .sprints
        .iter()
        .filter_map(|sprint| sprint.stories_by_status.get("done"))
        .flatten()
        .any(|story| story.status == "dropped");
    assert!(
        dropped_in_done,
        "`dropped` must fold into the done column (StoryStatus::board_bucket)"
    );
}

/// The dashboard's `progress` must be the same value the repository snapshot
/// serves, not an independently recomputed one.
#[test]
fn metrics_progress_is_the_snapshot_progress() {
    for fixture in fixtures() {
        let model = WebReadModel::build(fixture.root()).unwrap();
        let metrics = model.metrics();
        assert_eq!(
            serde_json::to_value(&metrics.progress).unwrap(),
            serde_json::to_value(&model.snapshot.progress).unwrap(),
        );
    }
}

/// Metrics and report must be identical to deriving them the old way, from
/// `list_all_stories` + `summarize_sprints`.
#[test]
fn metrics_and_report_match_the_separately_loaded_inputs() {
    for fixture in fixtures() {
        let model = WebReadModel::build(fixture.root()).unwrap();

        let stories = list_all_stories(fixture.root()).unwrap();
        let sprints = summarize_sprints(fixture.root()).unwrap();
        let expected_metrics = crate::metrics::compute_metrics(&model.snapshot, &stories, &sprints);
        assert_eq!(
            without_generated_at(serde_json::to_value(&expected_metrics).unwrap()),
            without_generated_at(serde_json::to_value(model.metrics()).unwrap()),
        );

        let current_sprint_name = sprints
            .iter()
            .find(|sprint| sprint.readme_status.as_deref() == Some("active"))
            .map(|sprint| sprint.sprint_name.as_str());
        let expected_report = crate::dto::WebReportDashboard::from(ReportDashboardDto::build(
            &stories,
            &sprints,
            current_sprint_name,
        ));
        assert_eq!(
            without_generated_at(serde_json::to_value(&expected_report).unwrap()),
            without_generated_at(serde_json::to_value(model.report()).unwrap()),
        );
    }
}

/// Epic detail must return the same epic and body the two-read implementation
/// produced.
#[test]
fn epic_detail_matches_find_epic_with_source() {
    let fixture = generate_backlog_fixture(&FixtureSpec::representative().with_stories(60));
    let snapshot = WebReadModel::build(fixture.root()).unwrap().into_snapshot();

    for epic_id in snapshot.epics.iter().map(|epic| epic.id.clone()) {
        let (epic, body) = WebReadModel::build(fixture.root())
            .unwrap()
            .epic_detail(&epic_id)
            .unwrap_or_else(|| panic!("epic detail missing for {epic_id}"));

        let expected_body = find_epic_with_source(fixture.root(), &epic_id)
            .unwrap()
            .map(|(_, source)| source.body)
            .unwrap_or_default();
        assert_eq!(expected_body, body, "epic body changed for {epic_id}");

        let mut expected_children = snapshot
            .epics
            .iter()
            .find(|candidate| candidate.id == epic_id)
            .unwrap()
            .stories
            .iter()
            .map(|story| story.id.clone())
            .collect::<Vec<_>>();
        expected_children.sort();
        let actual_children = epic
            .stories
            .iter()
            .map(|story| story.id.clone())
            .collect::<Vec<_>>();
        assert_eq!(expected_children, actual_children);
    }

    assert!(
        WebReadModel::build(fixture.root())
            .unwrap()
            .epic_detail("EP-does-not-exist")
            .is_none()
    );
}
