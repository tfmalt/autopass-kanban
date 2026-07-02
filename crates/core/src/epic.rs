use crate::config::*;
use crate::error::KanbanError;
use crate::lock::RepoLock;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::story::*;
use crate::util::*;
use crate::validate::{validate_local_timestamp_frontmatter, validate_markdown_date_frontmatter};

pub(crate) fn epic_status_warning(details: &EpicDetails) -> Option<String> {
    let epic_status = normalize_status_alias(&details.epic.status);
    let has_in_progress = details
        .stories_by_status
        .get("in-progress")
        .is_some_and(|stories| !stories.is_empty());
    if has_in_progress && matches!(epic_status.as_str(), "draft" | "todo") {
        Some(format!(
            "Epic status is `{}` but child stories are `in-progress`. Update the epic status to reflect active work.",
            details.epic.status
        ))
    } else {
        None
    }
}

pub fn find_epic(repo_root: impl AsRef<Path>, epic_id: &str) -> Result<Option<EpicDetails>> {
    let repo_root = repo_root.as_ref();
    let repository = read_repository(repo_root)?;
    let epic = find_epic_source(repo_root, epic_id)?;
    Ok(epic.map(|epic| epic_details_from_parts(repo_root, &repository, &epic)))
}

pub fn find_epic_with_source(
    repo_root: impl AsRef<Path>,
    epic_id: &str,
) -> Result<Option<(EpicDetails, Epic)>> {
    let repo_root = repo_root.as_ref();
    let repository = read_repository(repo_root)?;
    let epic = find_epic_source(repo_root, epic_id)?;
    Ok(epic.map(|epic| {
        let details = epic_details_from_parts(repo_root, &repository, &epic);
        (details, epic)
    }))
}

pub fn update_epic_frontmatter(
    repo_root: impl AsRef<Path>,
    epic_id: &str,
    updates: &[(String, String)],
) -> Result<EpicUpdateResult> {
    let config = load_kanban_config(repo_root)?;
    let _lock = RepoLock::acquire(&config.repo_root)?;
    let normalized_epic_id = epic_id.trim().to_ascii_uppercase();
    if updates.is_empty() {
        bail!("No epic frontmatter fields were provided.");
    }

    for (field, value) in updates {
        match field.as_str() {
            "priority" => validate_non_negative_integer_frontmatter(field, value)?,
            "planned_start" | "planned_end" => validate_markdown_date_frontmatter(field, value)?,
            "work_started" | "work_done" => validate_local_timestamp_frontmatter(field, value)?,
            _ => {}
        }
    }

    let epic = find_epic_source(&config.repo_root, &normalized_epic_id)?
        .ok_or_else(|| KanbanError::epic_not_found(&normalized_epic_id))?;
    let update_refs = updates
        .iter()
        .map(|(field, value)| (field.as_str(), Some(value.clone())))
        .collect::<Vec<_>>();
    let updated = upsert_frontmatter_markdown(&epic.markdown, &update_refs)?;
    atomic_write(&epic.file_path, &updated)
        .with_context(|| format!("write epic file {}", epic.file_path.display()))?;

    Ok(EpicUpdateResult {
        epic_id: epic
            .frontmatter
            .get("id")
            .cloned()
            .unwrap_or(normalized_epic_id),
        epic_path: epic.relative_path,
        updated_fields: updates.iter().map(|(field, _)| field.clone()).collect(),
    })
}

fn find_epic_source(repo_root: &Path, epic_id: &str) -> Result<Option<Epic>> {
    let normalized = epic_id.trim().to_ascii_uppercase();
    let mut matches = collect_epic_files(repo_root)?
        .into_iter()
        .map(|path| read_epic_file(path, repo_root))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|epic| {
            epic.frontmatter
                .get("id")
                .map(|id| id.eq_ignore_ascii_case(&normalized))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(matches.into_iter().next())
}

fn epic_details_from_parts(repo_root: &Path, repository: &Repository, epic: &Epic) -> EpicDetails {
    let overview = epic_overview(epic);
    let epic_id = overview.id.clone();
    let mut child_stories = repository
        .stories
        .iter()
        .filter(|story| {
            story
                .frontmatter
                .get("epic")
                .map(|value| value.eq_ignore_ascii_case(&epic_id))
                .unwrap_or(false)
        })
        .map(|story| story_overview(repo_root, story))
        .collect::<Vec<_>>();
    child_stories.sort_by(|left, right| left.id.cmp(&right.id));

    let mut stories_by_status = BTreeMap::<String, Vec<StoryOverview>>::new();
    let mut story_ids = Vec::new();
    for story in &child_stories {
        story_ids.push(story.id.clone());
        stories_by_status
            .entry(normalize_status_alias(&story.status))
            .or_default()
            .push(story.clone());
    }
    for stories in stories_by_status.values_mut() {
        stories.sort_by(|left, right| left.id.cmp(&right.id));
    }

    let mut details = EpicDetails {
        epic: overview,
        story_ids,
        stories_by_status,
        child_stories,
        warnings: Vec::new(),
        body: epic.body.clone(),
        business_context: extract_markdown_section(&epic.body, "Business Context"),
        business_value: extract_markdown_section(&epic.body, "Business Value"),
        scope: extract_markdown_section(&epic.body, "Scope"),
        acceptance_criteria: extract_markdown_section(&epic.body, "Acceptance Criteria"),
        non_functional_requirements: extract_markdown_section(
            &epic.body,
            "Non-Functional Requirements",
        ),
        dependencies: extract_markdown_section(&epic.body, "Dependencies"),
        definition_of_done: extract_markdown_section(&epic.body, "Definition of Done (Epic Level)")
            .or_else(|| extract_markdown_section(&epic.body, "Definition of Done")),
        notes_and_open_questions: extract_markdown_section(&epic.body, "Notes and Open Questions"),
    };
    if let Some(warning) = epic_status_warning(&details) {
        details.warnings.push(warning);
    }
    details
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use tempfile::tempdir;

    #[test]
    fn find_epic_exposes_child_story_progress_and_sections() {
        let (_fixture, repo_root) = build_fixture();
        let epic = find_epic(&repo_root, "EP-F1-06").unwrap().unwrap();

        assert_eq!(epic.epic.id, "EP-F1-06");
        assert!(epic.story_ids.contains(&"US-F1-052".to_string()));
        assert!(
            epic.stories_by_status
                .get("done")
                .into_iter()
                .flatten()
                .any(|story| story.id == "US-F1-052")
        );
        assert!(
            epic.acceptance_criteria
                .as_deref()
                .unwrap_or_default()
                .contains("current sprint can be understood")
        );
        assert!(epic.warnings.is_empty());
    }

    #[test]
    fn find_epic_returns_none_for_unknown_id() {
        let (_fixture, repo_root) = build_fixture();
        assert!(find_epic(&repo_root, "EP-F9-99").unwrap().is_none());
    }

    #[test]
    fn epic_status_warning_flags_draft_epic_with_in_progress_children() {
        let details = EpicDetails {
            epic: EpicOverview {
                id: "EP-F1-01".to_string(),
                title: "Platform".to_string(),
                status: "draft".to_string(),
                phase: Some("1".to_string()),
                owner: None,
                milestone: None,
                work_started: None,
                work_done: None,
                planned_start: None,
                planned_end: None,
                relative_path: PathBuf::from("delivery/backlog/phase-1/EP-F1-01.md"),
            },
            story_ids: vec!["US-F1-005".to_string()],
            stories_by_status: BTreeMap::from([(
                "in-progress".to_string(),
                vec![StoryOverview {
                    id: "US-F1-005".to_string(),
                    title: "Secrets".to_string(),
                    status: "in-progress".to_string(),
                    epic_id: Some("EP-F1-01".to_string()),
                    epic_title: Some("Platform".to_string()),
                    assignee: "TBD".to_string(),
                    story_points: "5".to_string(),
                    sprint: Some("S001.test".to_string()),
                    relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-005.md"),
                    task_summary: None,
                    task_count: 0,
                    work_started: None,
                    work_done: None,
                    planned_start: None,
                    planned_end: None,
                }],
            )]),
            child_stories: vec![],
            warnings: Vec::new(),
            body: String::new(),
            business_context: None,
            business_value: None,
            scope: None,
            acceptance_criteria: None,
            non_functional_requirements: None,
            dependencies: None,
            definition_of_done: None,
            notes_and_open_questions: None,
        };

        let warning = epic_status_warning(&details).expect("warning should exist");
        assert!(warning.contains("child stories are `in-progress`"));
    }

    #[test]
    fn update_epic_frontmatter_writes_priority() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let epic_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-099-test-epic.md",
            "id: EP-F1-099\ntype: epic\nstatus: draft\nphase: 1\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = update_epic_frontmatter(
            temp_root.path(),
            "EP-F1-099",
            &[("priority".to_string(), "10".to_string())],
        )
        .unwrap();

        let markdown = fs::read_to_string(epic_path).unwrap();
        assert_eq!(result.epic_id, "EP-F1-099");
        assert_eq!(result.updated_fields, vec!["priority"]);
        assert!(markdown.contains("priority: 10"));
    }

    #[test]
    fn update_epic_frontmatter_writes_lifecycle_fields() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let epic_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-097-test-epic.md",
            "id: EP-F1-097\ntype: epic\nstatus: draft\nphase: 1\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let result = update_epic_frontmatter(
            temp_root.path(),
            "EP-F1-097",
            &[
                ("planned_start".to_string(), "2026-06-15".to_string()),
                ("planned_end".to_string(), "2026-06-19".to_string()),
                (
                    "work_started".to_string(),
                    "2026-06-16T09:00:00+0200".to_string(),
                ),
                (
                    "work_done".to_string(),
                    "2026-06-18T17:00:00+0200".to_string(),
                ),
            ],
        )
        .unwrap();

        let markdown = fs::read_to_string(epic_path).unwrap();
        assert_eq!(result.epic_id, "EP-F1-097");
        assert_eq!(
            result.updated_fields,
            vec!["planned_start", "planned_end", "work_started", "work_done"]
        );
        assert!(markdown.contains("planned_start: 2026-06-15"));
        assert!(markdown.contains("planned_end: 2026-06-19"));
        assert!(markdown.contains("work_started: 2026-06-16T09:00:00+0200"));
        assert!(markdown.contains("work_done: 2026-06-18T17:00:00+0200"));
    }

    #[test]
    fn update_epic_frontmatter_rejects_negative_priority() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let epic_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-098-test-epic.md",
            "id: EP-F1-098\ntype: epic\nstatus: draft\nphase: 1\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let err = update_epic_frontmatter(
            temp_root.path(),
            "EP-F1-098",
            &[("priority".to_string(), "-1".to_string())],
        )
        .unwrap_err();

        assert!(err.to_string().contains("non-negative integer"));
        let markdown = fs::read_to_string(epic_path).unwrap();
        assert!(!markdown.contains("priority:"));
    }

    #[test]
    fn update_epic_frontmatter_rejects_invalid_lifecycle_values() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let epic_path = write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-096-test-epic.md",
            "id: EP-F1-096\ntype: epic\nstatus: draft\nphase: 1\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let err = update_epic_frontmatter(
            temp_root.path(),
            "EP-F1-096",
            &[("planned_start".to_string(), "2026/06/15".to_string())],
        )
        .unwrap_err();
        assert!(err.to_string().contains("YYYY-MM-DD"));

        let err = update_epic_frontmatter(
            temp_root.path(),
            "EP-F1-096",
            &[("work_started".to_string(), "2026-06-16".to_string())],
        )
        .unwrap_err();
        assert!(err.to_string().contains("ISO 8601"));

        let markdown = fs::read_to_string(epic_path).unwrap();
        assert!(!markdown.contains("planned_start:"));
        assert!(!markdown.contains("work_started:"));
    }

    #[test]
    fn update_epic_frontmatter_returns_error_for_unknown_id() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());

        let err = update_epic_frontmatter(
            temp_root.path(),
            "EP-F1-404",
            &[("priority".to_string(), "10".to_string())],
        )
        .unwrap_err();

        assert!(err.to_string().contains("Epic not found: EP-F1-404"));
    }
}
