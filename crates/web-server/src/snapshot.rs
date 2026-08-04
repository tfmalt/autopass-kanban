use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use kanban_core::*;

use crate::dto::*;

fn story_status(story: &WebStory) -> Option<StoryStatus> {
    StoryStatus::parse(&story.status)
}

fn story_is_done(story: &WebStory) -> bool {
    story_status(story) == Some(StoryStatus::Done)
}

pub(crate) fn load_repository_snapshot(repo_root: &Path) -> Result<RepositorySnapshot> {
    Ok(crate::read_model::WebReadModel::build(repo_root)?.into_snapshot())
}

pub(crate) fn web_story_from_core(repo_root: &Path, story: &kanban_core::Story) -> WebStory {
    let id = story.fields.id.clone();
    let tasks = story
        .task_file
        .as_ref()
        .map(|task_file| {
            task_file
                .tasks
                .iter()
                .map(web_task_from_core)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    WebStory {
        title: title_from_body(&story.body, "User Story"),
        status: story.fields.status_raw.clone(),
        phase: phase_from_id(&id, "US"),
        epic: story.fields.epic.clone(),
        sprint: story.fields.sprint.clone(),
        priority: story.fields.priority,
        story_points: story.fields.story_points,
        assignee: story.fields.assignee.clone(),
        assignees: story.fields.assignees.clone(),
        work_started: story.fields.work_started.clone(),
        work_done: story.fields.work_done.clone(),
        activated: story.fields.activated.clone(),
        created: story.fields.created.clone(),
        updated: story.fields.updated.clone(),
        relative_path: rel_to_root(repo_root, &story.relative_path),
        task_summary: summarize_web_tasks(&tasks),
        tasks,
        frontmatter: story.frontmatter.clone(),
        id,
    }
}

pub(crate) fn web_task_from_core(task: &kanban_core::Task) -> WebTask {
    WebTask {
        id: task.id.clone(),
        title: task.title.clone(),
        status: task.normalized_status.clone(),
        tags: task.tags.clone(),
        description: task.description.clone(),
    }
}

pub(crate) fn summarize_web_tasks(tasks: &[WebTask]) -> WebTaskSummary {
    let counts = TaskStatusCounts::count(tasks.iter().map(|task| task.status.as_str()));
    WebTaskSummary {
        // Unrecognized statuses fold into `todo`, same as before.
        todo: counts.todo + counts.other,
        in_progress: counts.in_progress,
        ready_for_qa: counts.ready_for_qa,
        done: counts.done,
        blocked: counts.blocked,
        total: counts.total,
    }
}

pub(crate) fn load_story_detail(repo_root: &Path, id: &str) -> Result<Option<(WebStory, String)>> {
    Ok(find_story_with_source(repo_root, id)?.map(|(_, source)| {
        let story = web_story_from_core(repo_root, &source);
        (story, source.body)
    }))
}

pub(crate) fn load_epic_detail(repo_root: &Path, id: &str) -> Result<Option<(WebEpic, String)>> {
    Ok(crate::read_model::WebReadModel::build(repo_root)?.epic_detail(id))
}

/// Group stories under their epics.
///
/// `epic_sources` are the already-read `EP-*.md` files in `collect_epic_files`
/// order. An epic file without an `id` frontmatter field is skipped, and when
/// several files claim the same id (case-insensitively) the lowest
/// `relative_path` wins — both behaviors are inherited from the `find_epic`
/// lookup this function replaced, and the fallback-epic path below still
/// materializes an epic for any `story.epic` value that has no epic file.
pub(crate) fn build_epics(epic_sources: &[Epic], stories: &[WebStory]) -> Vec<WebEpic> {
    let mut epics = BTreeMap::<String, WebEpic>::new();
    for source in epic_sources {
        let Some(raw_id) = source.frontmatter.get("id") else {
            continue;
        };
        let Some(canonical) = select_epic_source(epic_sources, raw_id) else {
            continue;
        };
        let overview = epic_overview(canonical);
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

/// Epic body markdown keyed by uppercased epic id, first file by path wins.
pub(crate) fn epic_body_index(epic_sources: &[Epic]) -> BTreeMap<String, String> {
    let mut bodies = BTreeMap::new();
    for source in epic_sources {
        let Some(id) = source.frontmatter.get("id") else {
            continue;
        };
        let Some(canonical) = select_epic_source(epic_sources, id) else {
            continue;
        };
        bodies.insert(id.trim().to_ascii_uppercase(), canonical.body.clone());
    }
    bodies
}

/// Bucket stories into board columns per sprint.
pub(crate) fn build_sprints(overviews: &[SprintOverview], stories: &[WebStory]) -> Vec<WebSprint> {
    let mut sprints = Vec::new();
    for overview in overviews {
        let mut by_status = BOARD_STATUSES
            .iter()
            .map(|status| ((*status).to_string(), Vec::<WebStory>::new()))
            .collect::<BTreeMap<_, _>>();
        for story in stories
            .iter()
            .filter(|story| story.sprint.as_deref() == Some(overview.sprint_name.as_str()))
        {
            let bucket_status = story_status(story)
                .map(StoryStatus::board_bucket)
                .map(|status| status.as_str())
                .unwrap_or_else(|| story.status.as_str());
            if let Some(bucket) = by_status.get_mut(bucket_status) {
                bucket.push(story.clone());
            }
        }
        for bucket in by_status.values_mut() {
            bucket.sort_by(|a, b| {
                priority_sort_key(a)
                    .cmp(&priority_sort_key(b))
                    .then_with(|| a.id.cmp(&b.id))
            });
        }
        sprints.push(WebSprint {
            name: overview.sprint_name.clone(),
            id: overview
                .sprint_name
                .split('.')
                .next()
                .unwrap_or(overview.sprint_name.as_str())
                .to_string(),
            headline: overview.headline.clone(),
            goal: overview.sprint_goal.clone(),
            start_date: Some(overview.start_date.clone()),
            end_date: Some(overview.end_date.clone()),
            status: overview.readme_status.clone(),
            wip_limit: overview.wip_limit,
            stories_by_status: by_status,
        });
    }
    sprints.sort_by(|a, b| a.name.cmp(&b.name));
    sprints
}

pub(crate) fn compute_progress(stories: &[WebStory]) -> ProjectProgress {
    let mut phases = BTreeMap::<String, PhaseSummary>::new();
    let mut done_points = 0;
    let mut total_points = 0;
    let mut done_stories = 0;
    let mut total_stories = 0;
    for story in stories {
        let points = story.story_points.unwrap_or(0);
        let phase = story.phase.clone().unwrap_or_else(|| "F?".to_string());
        let entry = phases.entry(phase.clone()).or_insert(PhaseSummary {
            phase,
            done_points: 0,
            total_points: 0,
            done_stories: 0,
            total_stories: 0,
        });
        if story_status(story).is_none_or(StoryStatus::counts_toward_scope) {
            entry.total_points += points;
            entry.total_stories += 1;
            total_points += points;
            total_stories += 1;
        }
        if story_is_done(story) {
            entry.done_points += points;
            entry.done_stories += 1;
            done_points += points;
            done_stories += 1;
        }
    }
    ProjectProgress {
        done_points,
        total_points,
        done_stories,
        total_stories,
        phases: phases.into_values().collect(),
    }
}

pub(crate) fn priority_sort_key(story: &WebStory) -> i64 {
    story.priority.unwrap_or(i64::MAX)
}

pub(crate) fn rel_to_root(repo_root: &Path, path: &Path) -> String {
    let path = if path.is_absolute() {
        path.strip_prefix(repo_root).unwrap_or(path)
    } else {
        path
    };
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn test_story(id: &str, status: &str, points: i64) -> WebStory {
        WebStory {
            id: id.to_string(),
            title: id.to_string(),
            status: status.to_string(),
            phase: Some("F1".to_string()),
            epic: Some("EP-F1-01".to_string()),
            sprint: Some("S001.current".to_string()),
            priority: None,
            story_points: Some(points),
            assignee: None,
            assignees: Vec::new(),
            work_started: None,
            work_done: None,
            activated: None,
            created: None,
            updated: None,
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
            frontmatter: BTreeMap::new(),
        }
    }

    #[test]
    fn compute_progress_excludes_dropped_from_scope_totals() {
        let progress = compute_progress(&[
            test_story("US-F1-001", "done", 5),
            test_story("US-F1-002", "dropped", 3),
            test_story("US-F1-003", "todo", 4),
        ]);

        assert_eq!(progress.done_points, 5);
        assert_eq!(progress.total_points, 9);
        assert_eq!(progress.done_stories, 1);
        assert_eq!(progress.total_stories, 2);
        assert_eq!(progress.phases[0].done_points, 5);
        assert_eq!(progress.phases[0].total_points, 9);
        assert_eq!(progress.phases[0].done_stories, 1);
        assert_eq!(progress.phases[0].total_stories, 2);
    }
}
