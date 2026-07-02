use super::*;
use crate::dto::{BOARD_STATUSES, WebSprint, WebStory, WebTaskSummary};

fn test_story(
    id: &str,
    status: &str,
    story_points: i64,
    created: Option<&str>,
    work_done: Option<&str>,
) -> WebStory {
    WebStory {
        id: id.to_string(),
        title: id.to_string(),
        status: status.to_string(),
        phase: Some("F1".to_string()),
        epic: Some("EP-F1-01".to_string()),
        sprint: Some("S001.current".to_string()),
        priority: None,
        story_points: Some(story_points),
        assignee: None,
        assignees: Vec::new(),
        work_started: created.map(str::to_string),
        work_done: work_done.map(str::to_string),
        activated: created.map(str::to_string),
        created: created.map(str::to_string),
        updated: work_done.map(str::to_string),
        relative_path: "story.md".to_string(),
        tasks: Vec::new(),
        task_summary: WebTaskSummary {
            todo: 0,
            in_progress: 0,
            ready_for_qa: 0,
            done: 0,
            blocked: 0,
            total: 0,
        },
        frontmatter: std::collections::BTreeMap::new(),
    }
}

fn test_sprint(status: &str, stories: Vec<WebStory>) -> WebSprint {
    test_sprint_with_start(status, "2026-06-01", stories)
}

fn test_sprint_with_start(status: &str, start_date: &str, stories: Vec<WebStory>) -> WebSprint {
    test_sprint_with_dates(status, start_date, "2026-06-05", stories)
}

fn test_sprint_with_dates(
    status: &str,
    start_date: &str,
    end_date: &str,
    stories: Vec<WebStory>,
) -> WebSprint {
    let mut stories_by_status = BOARD_STATUSES
        .iter()
        .map(|name| ((*name).to_string(), Vec::<WebStory>::new()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for story in stories {
        stories_by_status
            .get_mut(if story.status == "dropped" {
                "done"
            } else {
                &story.status
            })
            .expect("known status bucket")
            .push(story);
    }
    WebSprint {
        name: "S001.current".to_string(),
        id: "S001".to_string(),
        headline: "current".to_string(),
        goal: None,
        start_date: Some(start_date.to_string()),
        end_date: Some(end_date.to_string()),
        status: Some(status.to_string()),
        wip_limit: None,
        stories_by_status,
    }
}

#[test]
fn build_burnup_starts_at_earliest_work_started_date() {
    let done = test_story(
        "US-F1-001",
        "done",
        5,
        Some("2026-06-01T09:00:00+0200"),
        Some("2026-06-03T12:00:00+0200"),
    );
    let todo = test_story(
        "US-F1-002",
        "todo",
        8,
        Some("2026-06-01T09:00:00+0200"),
        None,
    );
    let early_created_only = WebStory {
        created: Some("2026-03-30T00:00:00+0200".to_string()),
        activated: Some("2026-03-30T00:00:00+0200".to_string()),
        work_started: None,
        ..test_story("US-F2-001", "draft", 5, None, None)
    };
    let rows = build_burnup(&[done.clone(), todo, early_created_only], &[]);
    assert_eq!(
        rows,
        vec![
            BurnupPoint {
                date: "2026-06-01".to_string(),
                completed: 0,
                scope: 0
            },
            BurnupPoint {
                date: "2026-06-03".to_string(),
                completed: 5,
                scope: 0
            },
            BurnupPoint {
                date: Local::now().date_naive().to_string(),
                completed: 5,
                scope: 0
            }
        ]
    );
}

#[test]
fn build_burnup_scope_steps_with_sprint_commitments() {
    let today = Local::now().date_naive();
    let sprint_zero_start = (today - Days::new(10)).to_string();
    let sprint_one_start = (today - Days::new(2)).to_string();
    let work_started = (today - Days::new(9)).to_string();
    let done = test_story(
        "US-F1-001",
        "done",
        5,
        Some(&format!("{work_started}T09:00:00+0200")),
        Some(&format!("{}T12:00:00+0200", (today - Days::new(7)))),
    );
    let next = test_story("US-F1-002", "todo", 8, None, None);
    let rows = build_burnup(
        &[done.clone(), next.clone()],
        &[
            test_sprint_with_start("closed", &sprint_zero_start, vec![done]),
            test_sprint_with_start("active", &sprint_one_start, vec![next]),
        ],
    );
    assert_eq!(
        rows.first(),
        Some(&BurnupPoint {
            date: work_started,
            completed: 0,
            scope: 5
        })
    );
    assert!(
        rows.iter()
            .any(|row| row.date == sprint_one_start && row.scope == 13)
    );
    assert_eq!(
        rows.last().map(|row| row.date.clone()),
        Some(today.to_string())
    );
}

#[test]
fn build_burnup_includes_past_sprint_end_dates_as_scope_anchors() {
    let today = Local::now().date_naive();
    let sprint_start = (today - Days::new(6)).to_string();
    let sprint_end = (today - Days::new(3)).to_string();
    let work_started = (today - Days::new(5)).to_string();
    let done = test_story(
        "US-F1-001",
        "done",
        5,
        Some(&format!("{work_started}T09:00:00+0200")),
        Some(&format!("{}T12:00:00+0200", (today - Days::new(4)))),
    );
    let sprint_story = done.clone();
    let rows = build_burnup(
        std::slice::from_ref(&done),
        &[test_sprint_with_dates(
            "closed",
            &sprint_start,
            &sprint_end,
            vec![sprint_story],
        )],
    );
    assert!(
        rows.iter()
            .any(|row| row.date == sprint_end && row.scope == 5)
    );
}

#[test]
fn build_burndown_uses_active_sprint_story_progress() {
    let done = test_story(
        "US-F1-001",
        "done",
        5,
        Some("2026-06-01T09:00:00+0200"),
        Some("2026-06-03T12:00:00+0200"),
    );
    let todo = test_story(
        "US-F1-002",
        "todo",
        8,
        Some("2026-06-01T09:00:00+0200"),
        None,
    );
    let rows = build_burndown(&[test_sprint("active", vec![done, todo])]);
    assert_eq!(
        rows.first(),
        Some(&BurndownPoint {
            date: "2026-06-01".to_string(),
            remaining: 13,
            ideal: 13
        })
    );
    assert!(
        rows.iter()
            .any(|row| row.date == "2026-06-03" && row.remaining == 8)
    );
}

#[test]
fn build_burnup_does_not_count_dropped_points_as_completed() {
    let dropped = test_story(
        "US-F1-003",
        "dropped",
        3,
        Some("2026-06-01T09:00:00+0200"),
        Some("2026-06-04T12:00:00+0200"),
    );

    let rows = build_burnup(&[dropped], &[]);
    assert!(rows.iter().all(|row| row.completed == 0));
}

#[test]
fn build_burndown_excludes_dropped_points_from_scope() {
    let dropped = test_story(
        "US-F1-003",
        "dropped",
        3,
        Some("2026-06-01T09:00:00+0200"),
        Some("2026-06-03T12:00:00+0200"),
    );
    let todo = test_story(
        "US-F1-004",
        "todo",
        8,
        Some("2026-06-01T09:00:00+0200"),
        None,
    );

    let rows = build_burndown(&[test_sprint("active", vec![dropped, todo])]);
    assert_eq!(rows.first().map(|row| row.remaining), Some(8));
}
