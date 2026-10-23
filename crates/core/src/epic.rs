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
    let config = load_kanban_config(repo_root)?;
    let repository = read_repository_with_config(&config)?;
    find_epic_in_repository(&repository, &config, epic_id)
}

pub fn find_epic_with_source(
    repo_root: impl AsRef<Path>,
    epic_id: &str,
) -> Result<Option<(EpicDetails, Epic)>> {
    let config = load_kanban_config(repo_root)?;
    let repository = read_repository_with_config(&config)?;
    let epic = find_epic_source_with_config(&config, epic_id)?;
    Ok(epic.map(|epic| {
        let details = epic_details_from_parts(&config.repo_root, &repository, &epic);
        (details, epic)
    }))
}

/// Config-aware, non-reloading variant of [`find_epic`].
///
/// Callers that already parsed the repository must use this: [`find_epic`]
/// performs a full `read_repository` plus a full epic-file rescan, so calling it
/// inside a loop over epics is quadratic in repository size.
pub fn find_epic_in_repository(
    repository: &Repository,
    config: &KanbanConfig,
    epic_id: &str,
) -> Result<Option<EpicDetails>> {
    let epic = find_epic_source_with_config(config, epic_id)?;
    Ok(epic.map(|epic| epic_details_from_parts(&config.repo_root, repository, &epic)))
}

/// Read every epic source in the backlog exactly once, in `collect_epic_files`
/// order.
///
/// Callers that need more than one epic must read the sources once and resolve
/// against the returned slice with [`select_epic_source`]; resolving each epic
/// through [`find_epic`] rescans the whole backlog per epic.
pub fn read_epic_sources(config: &KanbanConfig) -> Result<Vec<Epic>> {
    collect_epic_files_with_config(config)?
        .into_iter()
        .map(|path| read_epic_file_with_config(path, config))
        .collect()
}

pub fn list_all_epic_overviews(repo_root: impl AsRef<Path>) -> Result<Vec<EpicOverview>> {
    let config = load_kanban_config(repo_root)?;
    Ok(read_epic_sources(&config)?
        .iter()
        .map(epic_overview)
        .collect())
}

/// Resolve the canonical epic source for `epic_id` from already-read sources.
///
/// Matching is case-insensitive on the frontmatter `id`; ties are broken by the
/// lowest `relative_path`. Sources without an `id` never match, which is what
/// makes an epic file with no `id` frontmatter invisible to epic lookups.
pub fn select_epic_source<'a>(sources: &'a [Epic], epic_id: &str) -> Option<&'a Epic> {
    let normalized = epic_id.trim().to_ascii_uppercase();
    sources
        .iter()
        .filter(|epic| {
            epic.frontmatter
                .get("id")
                .map(|id| id.eq_ignore_ascii_case(&normalized))
                .unwrap_or(false)
        })
        .min_by(|left, right| left.relative_path.cmp(&right.relative_path))
}

/// Build [`EpicDetails`] for an already-read epic source and repository.
pub fn epic_details_from_source(
    repository: &Repository,
    config: &KanbanConfig,
    epic: &Epic,
) -> EpicDetails {
    epic_details_from_parts(&config.repo_root, repository, epic)
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
        return Err(
            KanbanError::invalid_argument("No epic frontmatter fields were provided.").into(),
        );
    }

    for (field, value) in updates {
        match field.as_str() {
            "priority" => validate_non_negative_integer_frontmatter(field, value)?,
            "planned_start" | "planned_end" => validate_markdown_date_frontmatter(field, value)?,
            "work_started" | "work_done" => validate_local_timestamp_frontmatter(field, value)?,
            _ => {}
        }
    }

    let epic = find_epic_source_with_config(&config, &normalized_epic_id)?
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

pub fn create_epic(
    repo_root: impl AsRef<Path>,
    input: &CreateEpicInput,
) -> Result<CreateEpicResult> {
    let config = load_kanban_config(repo_root)?;
    let _lock = RepoLock::acquire(&config.repo_root)?;
    if !config.features().epics {
        return Err(KanbanError::feature_disabled("epics").into());
    }
    let title = input.title.trim();
    let slug = slugify_headline(title);
    if slug.is_empty() || title.contains(['\n', '\r']) {
        return Err(KanbanError::invalid_argument(
            "Epic title must be a single non-empty line containing ASCII letters or numbers.",
        )
        .into());
    }
    for value in [&input.owner, &input.milestone] {
        if value.trim().is_empty() || value.contains(['\n', '\r']) {
            return Err(KanbanError::invalid_argument(
                "Epic owner and milestone must be non-empty single-line values.",
            )
            .into());
        }
    }
    let sources = read_epic_sources(&config)?;
    let id_pattern = Regex::new(r"^EP-(?:F([1-9][0-9]*)-([0-9]{2,})|([0-9]{3,}))$")?;
    let supplied = input
        .id
        .as_deref()
        .map(|value| value.trim().to_ascii_uppercase());
    let id_phase = supplied
        .as_deref()
        .map(|id| {
            let captures = id_pattern.captures(id).ok_or_else(|| {
                KanbanError::invalid_argument("Epic id must be EP-NNN or EP-Fn-NN.")
            })?;
            captures
                .get(1)
                .map(|value| value.as_str().parse::<u32>())
                .transpose()
                .map_err(anyhow::Error::from)
        })
        .transpose()?
        .flatten();
    if input.phase == Some(0)
        || input
            .phase
            .zip(id_phase)
            .is_some_and(|(requested, actual)| requested != actual)
    {
        return Err(
            KanbanError::invalid_argument("Epic phase must be positive and match the id.").into(),
        );
    }
    let phase = input.phase.or(id_phase);
    if config.features().phases && phase.is_none() {
        return Err(
            KanbanError::invalid_argument("A phase is required when phases are enabled.").into(),
        );
    }
    let prefix = if config.features().phases {
        format!("EP-F{}-", phase.unwrap_or_default())
    } else {
        "EP-".to_string()
    };
    let next_number = sources
        .iter()
        .filter_map(|epic| epic.frontmatter.get("id"))
        .filter_map(|id| id.strip_prefix(&prefix))
        .filter_map(|value| value.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| KanbanError::invalid_argument("Epic id range exhausted."))?;
    let id = supplied.unwrap_or_else(|| {
        if config.features().phases {
            format!("{prefix}{next_number:02}")
        } else {
            format!("{prefix}{next_number:03}")
        }
    });
    if config.features().phases && id_phase.is_none() && input.id.is_some() {
        return Err(
            KanbanError::invalid_argument("Phased repositories require an EP-Fn-NN id.").into(),
        );
    }
    if select_epic_source(&sources, &id).is_some() {
        return Err(KanbanError::invalid_argument(format!("Epic already exists: {id}")).into());
    }
    let template_path = [config.repo_root.join("templates/TEMPLATE-epic-en.md"), config.backlog_path().join("templates/TEMPLATE-epic-en.md")]
        .into_iter().find(|path| path.is_file()).ok_or_else(|| KanbanError::invalid_argument("Epic template TEMPLATE-epic-en.md was not found in templates/ or the backlog templates/ directory."))?;
    let template = fs::read_to_string(template_path)?;
    let timestamp = current_timestamp_string();
    let phase_value = phase.unwrap_or(1).to_string();
    let markdown = upsert_frontmatter_markdown(
        &template,
        &[
            ("id", Some(id.clone())),
            ("type", Some("epic".to_string())),
            ("status", Some("draft".to_string())),
            ("phase", Some(phase_value.clone())),
            ("owner", Some(serde_json::to_string(input.owner.trim())?)),
            (
                "milestone",
                Some(serde_json::to_string(input.milestone.trim())?),
            ),
            ("created", Some(timestamp.clone())),
            ("updated", Some(timestamp)),
        ],
    )?;
    let heading = Regex::new(r"(?m)^# (?:Epic:.*|[^\n]+)$")?;
    let markdown = heading
        .replacen(&markdown, 1, |_: &regex::Captures<'_>| {
            format!("# Epic: {title}")
        })
        .into_owned();
    let backlog = config.backlog_path();
    let directory = if config.features().phases {
        let marker = format!("phase-{phase_value}");
        let candidates = fs::read_dir(&backlog)?
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name == marker || name.starts_with(&format!("{marker}-"))
                })
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(KanbanError::invalid_argument(
                "The requested phase must resolve to exactly one existing phase directory.",
            )
            .into());
        }
        let number = id.rsplit('-').next().unwrap_or_default();
        ensure_path_inside(&backlog, &candidates[0].join(format!("{number}.{slug}")))?
    } else {
        ensure_path_inside(&backlog, &backlog)?
    };
    if config.features().phases && directory.exists() {
        return Err(KanbanError::invalid_argument(
            "The destination epic directory already exists.",
        )
        .into());
    }
    let new_directory = !directory.exists();
    if new_directory {
        fs::create_dir(&directory)?;
    }
    let result = (|| {
        let path = ensure_path_inside(&backlog, &directory.join(format!("{id}-{slug}.md")))?;
        if path.exists() {
            return Err(
                KanbanError::invalid_argument("The destination epic file already exists.").into(),
            );
        }
        atomic_write(&path, &markdown)?;
        Ok(CreateEpicResult {
            epic_id: id,
            epic_path: relative_path(&config.repo_root, &path),
        })
    })();
    if result.is_err() && new_directory {
        let _ = fs::remove_dir(&directory);
    }
    result
}

fn find_epic_source_with_config(config: &KanbanConfig, epic_id: &str) -> Result<Option<Epic>> {
    let sources = read_epic_sources(config)?;
    Ok(select_epic_source(&sources, epic_id).cloned())
}

fn epic_details_from_parts(repo_root: &Path, repository: &Repository, epic: &Epic) -> EpicDetails {
    let overview = epic_overview(epic);
    let epic_id = overview.id.clone();
    let mut child_stories = repository
        .stories
        .iter()
        .filter(|story| {
            story
                .fields
                .epic
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(&epic_id))
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
    fn create_epic_places_unique_id_in_phase_and_preserves_existing_files() {
        let (_fixture, root) = build_fixture();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("templates/TEMPLATE-epic-en.md"), "---\nid: EP-XXX\ntype: epic\nstatus: draft\n---\n\n# Epic: Placeholder\n\n## Child User Stories\n").unwrap();
        let input = CreateEpicInput {
            id: Some("EP-F1-07".into()),
            title: "Agreements".into(),
            phase: Some(1),
            owner: "Domain owner".into(),
            milestone: "MP1".into(),
        };
        let result = create_epic(&root, &input).unwrap();
        assert_eq!(result.epic_id, "EP-F1-07");
        assert_eq!(
            result.epic_path,
            PathBuf::from(
                "delivery/backlog/phase-1-scaffolding/07.agreements/EP-F1-07-agreements.md"
            )
        );
        let bytes = fs::read(root.join(&result.epic_path)).unwrap();
        assert!(create_epic(&root, &input).is_err());
        assert_eq!(fs::read(root.join(result.epic_path)).unwrap(), bytes);
    }

    #[test]
    fn create_epic_supports_flat_layout_and_safe_scalar_metadata() {
        let (_fixture, root) = build_fixture();
        let settings = root.join(".kanban/settings.json");
        let mut config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        config["paths"]["features"]["phases"] = false.into();
        fs::write(settings, serde_json::to_string(&config).unwrap()).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(
            root.join("templates/TEMPLATE-epic-en.md"),
            "---\nid: EP-XXX\n---\n\n# Epic: Placeholder\n",
        )
        .unwrap();
        let input = CreateEpicInput {
            id: None,
            title: "Flat epic".into(),
            phase: None,
            owner: "Team: operations".into(),
            milestone: "MP2".into(),
        };
        let result = create_epic(&root, &input).unwrap();
        assert_eq!(result.epic_id, "EP-001");
        assert_eq!(
            result.epic_path,
            PathBuf::from("delivery/backlog/EP-001-flat-epic.md")
        );
        let text = fs::read_to_string(root.join(result.epic_path)).unwrap();
        let metadata: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(text.split("---").nth(1).unwrap()).unwrap();
        assert_eq!(metadata["owner"].as_str(), Some("Team: operations"));
        assert!(
            create_epic(
                &root,
                &CreateEpicInput {
                    owner: "bad\nvalue".into(),
                    ..input
                }
            )
            .is_err()
        );
    }

    #[test]
    fn create_epic_rejects_inconsistent_or_unsafe_ids_before_creating_folders() {
        let (_fixture, root) = build_fixture();
        let input = CreateEpicInput {
            id: Some("EP-F4-08".into()),
            title: "Agreements".into(),
            phase: Some(1),
            owner: "Owner".into(),
            milestone: "MP4".into(),
        };
        assert!(create_epic(&root, &input).is_err());
        assert!(
            create_epic(
                &root,
                &CreateEpicInput {
                    id: Some("../../escape".into()),
                    ..input
                }
            )
            .is_err()
        );
        assert!(
            !root
                .join("delivery/backlog/phase-1-scaffolding/08.agreements")
                .exists()
        );
    }

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
                priority: None,
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
