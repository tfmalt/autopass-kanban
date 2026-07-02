use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use kanban_core::*;

use crate::dto::*;
use crate::team::parse_assignees;

fn counts_toward_scope(story: &WebStory) -> bool {
    story.status != "dropped"
}

fn board_bucket_status(story: &WebStory) -> &str {
    if story.status == "dropped" {
        "done"
    } else {
        story.status.as_str()
    }
}

pub(crate) fn load_repository_snapshot(repo_root: &Path) -> Result<RepositorySnapshot> {
    let repository = read_repository(repo_root)?;
    let mut stories = repository
        .stories
        .iter()
        .map(|story| web_story_from_core(&repository.repo_root, story))
        .collect::<Vec<_>>();
    stories.sort_by(|a, b| a.id.cmp(&b.id));
    let epics = load_epics(&repository.repo_root, &stories)?;
    let sprints = load_sprints(&repository.repo_root, &stories)?;
    let progress = compute_progress(&stories);
    Ok(RepositorySnapshot {
        stories,
        epics,
        sprints,
        progress,
    })
}

pub(crate) fn web_story_from_core(repo_root: &Path, story: &kanban_core::Story) -> WebStory {
    let id = story.frontmatter.get("id").cloned().unwrap_or_default();
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
    let assignee = empty_to_none(story.frontmatter.get("assignee"));
    let assignees = assignee.as_deref().map(parse_assignees).unwrap_or_default();
    WebStory {
        title: title_from_body(&story.body, "User Story"),
        status: story.frontmatter.get("status").cloned().unwrap_or_default(),
        phase: phase_from_id(&id, "US"),
        epic: empty_to_none(story.frontmatter.get("epic")),
        sprint: empty_to_none(story.frontmatter.get("sprint")),
        priority: story
            .frontmatter
            .get("priority")
            .and_then(|value| parse_non_negative_i64(value)),
        story_points: story
            .frontmatter
            .get("story_points")
            .and_then(|value| parse_i64(value)),
        assignee,
        assignees,
        work_started: empty_to_none(story.frontmatter.get("work_started")),
        work_done: empty_to_none(story.frontmatter.get("work_done")),
        activated: empty_to_none(story.frontmatter.get("activated")),
        created: empty_to_none(story.frontmatter.get("created")),
        updated: empty_to_none(story.frontmatter.get("updated")),
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
    let mut summary = WebTaskSummary {
        todo: 0,
        in_progress: 0,
        ready_for_qa: 0,
        done: 0,
        blocked: 0,
        total: tasks.len(),
    };
    for task in tasks {
        match task.status.as_str() {
            "in-progress" => summary.in_progress += 1,
            "ready-for-qa" => summary.ready_for_qa += 1,
            "done" => summary.done += 1,
            "blocked" => summary.blocked += 1,
            _ => summary.todo += 1,
        }
    }
    summary
}

pub(crate) fn load_story_detail(repo_root: &Path, id: &str) -> Result<Option<(WebStory, String)>> {
    Ok(find_story_with_source(repo_root, id)?.map(|(_, source)| {
        let story = web_story_from_core(repo_root, &source);
        (story, source.body)
    }))
}

pub(crate) fn load_epic_detail(repo_root: &Path, id: &str) -> Result<Option<(WebEpic, String)>> {
    let repository = load_repository_snapshot(repo_root)?;
    let Some(mut epic) = repository
        .epics
        .into_iter()
        .find(|epic| epic.id.eq_ignore_ascii_case(id))
    else {
        return Ok(None);
    };
    let source = find_epic_with_source(repo_root, id)?;
    let body = source.map(|(_, source)| source.body).unwrap_or_default();
    epic.stories.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Some((epic, body)))
}

pub(crate) fn load_epics(repo_root: &Path, stories: &[WebStory]) -> Result<Vec<WebEpic>> {
    let mut epics = BTreeMap::<String, WebEpic>::new();
    for path in collect_epic_files(repo_root)? {
        let source = read_epic_file(&path, repo_root)?;
        let id = source.frontmatter.get("id").cloned().unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        epics.insert(
            id.clone(),
            WebEpic {
                title: title_from_body(&source.body, "Epic"),
                phase: phase_from_id(&id, "EP").unwrap_or_else(|| "F?".to_string()),
                priority: source
                    .frontmatter
                    .get("priority")
                    .and_then(|value| parse_non_negative_i64(value)),
                planned_start: empty_to_none(source.frontmatter.get("planned_start")),
                planned_end: empty_to_none(source.frontmatter.get("planned_end")),
                work_started: empty_to_none(source.frontmatter.get("work_started")),
                work_done: empty_to_none(source.frontmatter.get("work_done")),
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
    Ok(epics.into_values().collect())
}

pub(crate) fn load_sprints(repo_root: &Path, stories: &[WebStory]) -> Result<Vec<WebSprint>> {
    let config = load_kanban_config(repo_root)?;
    let mut sprints = Vec::new();
    let Ok(entries) = fs::read_dir(config.sprints_path()) else {
        return Ok(sprints);
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !stem.starts_with('S') || !stem.contains('.') {
            continue;
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("read sprint file {}", path.display()))?;
        let parsed = parse_frontmatter(&content);
        let mut by_status = BOARD_STATUSES
            .iter()
            .map(|status| ((*status).to_string(), Vec::<WebStory>::new()))
            .collect::<BTreeMap<_, _>>();
        for story in stories
            .iter()
            .filter(|story| story.sprint.as_deref() == Some(stem))
        {
            if let Some(bucket) = by_status.get_mut(board_bucket_status(story)) {
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
            name: stem.to_string(),
            id: parsed
                .frontmatter
                .get("sprint")
                .cloned()
                .unwrap_or_else(|| stem.split('.').next().unwrap_or(stem).to_string()),
            headline: parsed
                .frontmatter
                .get("headline")
                .cloned()
                .unwrap_or_default(),
            goal: extract_section(&parsed.body, "Sprint Goal"),
            start_date: empty_to_none(parsed.frontmatter.get("start_date")),
            end_date: empty_to_none(parsed.frontmatter.get("end_date")),
            status: empty_to_none(parsed.frontmatter.get("status")),
            wip_limit: parsed
                .frontmatter
                .get("wip_limit")
                .and_then(|value| parse_non_negative_i64(value)),
            stories_by_status: by_status,
        });
    }
    sprints.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sprints)
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
        if counts_toward_scope(story) {
            entry.total_points += points;
            entry.total_stories += 1;
            total_points += points;
            total_stories += 1;
        }
        if story.status == "done" {
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

pub(crate) fn title_from_body(body: &str, prefix: &str) -> String {
    body.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(|title| {
            title
                .trim()
                .strip_prefix(&format!("{prefix}: "))
                .unwrap_or(title.trim())
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

pub(crate) fn phase_from_id(id: &str, prefix: &str) -> Option<String> {
    let marker = format!("{prefix}-F");
    let start = id.to_ascii_uppercase().find(&marker)? + prefix.len() + 1;
    let rest = &id[start..];
    let end = rest.find('-').unwrap_or(rest.len());
    let phase = &rest[..end];
    (!phase.is_empty()).then(|| phase.to_ascii_uppercase())
}

pub(crate) fn empty_to_none(value: Option<&String>) -> Option<String> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "~" && *value != "null")
        .map(str::to_string)
}

pub(crate) fn parse_i64(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok()
}

pub(crate) fn parse_non_negative_i64(value: &str) -> Option<i64> {
    parse_i64(value).filter(|value| *value >= 0)
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

pub(crate) fn extract_section(body: &str, heading: &str) -> Option<String> {
    let marker = format!("## {heading}");
    let start = body.find(&marker)? + marker.len();
    let rest = &body[start..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
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
