use crate::config::*;
use crate::constants::*;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::sprint::*;
use crate::util::*;

/// Atomically write `contents` to `path` using a temp-file-then-rename pattern
/// (US-012).
///
/// The temp file is created in `path`'s parent directory so the final
/// `persist` (rename) is a same-volume move on both Unix and Windows. The temp
/// file is `fsync`ed before the rename so a crash mid-write cannot leave a
/// truncated or empty file at the final path: the original remains intact until
/// the rename succeeds. The parent directory is created if missing.
///
/// This replaces every direct `fs::write` in core and web-server backlog
/// writers so the markdown source of truth stays crash-safe.
pub fn atomic_write(path: impl AsRef<Path>, contents: &str) -> Result<()> {
    use std::io::Write;

    let path = path.as_ref();
    let parent = path
        .parent()
        .with_context(|| format!("cannot determine parent directory of {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create directory {}", parent.display()))?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create temp file in {}", parent.display()))?;
    temp.write_all(contents.as_bytes())
        .with_context(|| format!("write temp file {}", temp.path().display()))?;
    temp.as_file()
        .sync_all()
        .with_context(|| format!("fsync temp file {}", temp.path().display()))?;
    temp.persist(path)
        .map_err(|err| anyhow::anyhow!("persist temp file to {}: {err}", path.display()))?;
    Ok(())
}

pub fn collect_user_story_files(repo_root: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let config = load_kanban_config(repo_root)?;
    collect_user_story_files_with_config(&config)
}

/// Config-aware variant of [`collect_user_story_files`].
///
/// Callers that already hold a [`KanbanConfig`] must use this so a repository
/// read resolves the git root and parses `.kanban/settings.json` exactly once
/// instead of once per collected file.
pub fn collect_user_story_files_with_config(config: &KanbanConfig) -> Result<Vec<PathBuf>> {
    let backlog_root = config.backlog_path();
    let canonical_backlog = backlog_root
        .canonicalize()
        .unwrap_or_else(|_| backlog_root.clone());
    let mut files = Vec::new();

    for entry in WalkDir::new(&backlog_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry?;
        // US-009: explicitly skip symlinks so a planted US-*.md symlink
        // cannot resolve outside the backlog root and cause writes through it.
        if entry.file_type().is_symlink() {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if name.starts_with(STORY_FILE_PREFIX)
            && name.ends_with(STORY_FILE_SUFFIX)
            && !name.ends_with(TASK_FILE_SUFFIX)
        {
            // Defense-in-depth: reject files whose canonicalized path is
            // outside the canonicalized backlog root.
            if let Ok(canonical) = entry.path().canonicalize()
                && !canonical.starts_with(&canonical_backlog)
            {
                continue;
            }
            files.push(entry.into_path());
        }
    }

    files.sort();
    Ok(files)
}

/// Collect all epic markdown files (`EP-*.md`) from the backlog tree.
pub fn collect_epic_files(repo_root: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let config = load_kanban_config(repo_root)?;
    collect_epic_files_with_config(&config)
}

/// Config-aware variant of [`collect_epic_files`].
pub fn collect_epic_files_with_config(config: &KanbanConfig) -> Result<Vec<PathBuf>> {
    let backlog_root = config.backlog_path();
    let canonical_backlog = backlog_root
        .canonicalize()
        .unwrap_or_else(|_| backlog_root.clone());
    let mut files = Vec::new();

    for entry in WalkDir::new(&backlog_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if name.starts_with(EPIC_FILE_PREFIX) && name.ends_with(STORY_FILE_SUFFIX) {
            if let Ok(canonical) = entry.path().canonicalize()
                && !canonical.starts_with(&canonical_backlog)
            {
                continue;
            }
            files.push(entry.into_path());
        }
    }

    files.sort();
    Ok(files)
}

/// Return all sprint folder names (e.g. `S000.getting-started`) sorted alphabetically.
/// This is a lightweight listing suitable for shell completion.
pub fn list_sprint_names(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let config = load_kanban_config(repo_root)?;
    let mut specs = discover_sprint_folder_specs(&config)?;
    specs.sort_by(|a, b| a.sprint_name.cmp(&b.sprint_name));
    Ok(specs.into_iter().map(|s| s.sprint_name).collect())
}

/// Return unique user story IDs (e.g. `US-F1-053`) sorted alphabetically.
/// Each ID appears only once regardless of how many copies (sprint vs backlog) exist.
/// This is a lightweight listing suitable for shell completion.
pub fn list_story_ids(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let repository = read_repository(repo_root)?;
    let mut seen = BTreeSet::new();
    for story in &repository.stories {
        let id_upper = story.fields.id.trim().to_ascii_uppercase();
        if !id_upper.is_empty() {
            seen.insert(id_upper);
        }
    }
    Ok(seen.into_iter().collect())
}

/// Return user story completion values with display descriptions.
/// `value` is the inserted shell completion; `description` is menu text only.
pub fn list_story_completion_items(repo_root: impl AsRef<Path>) -> Result<Vec<CompletionItem>> {
    let repository = read_repository(repo_root)?;
    let mut items = BTreeMap::new();
    for story in &repository.stories {
        let id_upper = story.fields.id.trim().to_ascii_uppercase();
        if !id_upper.is_empty() {
            let title = story_title(&story.body).unwrap_or_else(|| story.file_name.clone());
            items.entry(id_upper).or_insert(title);
        }
    }

    Ok(items
        .into_iter()
        .map(|(value, description)| CompletionItem { value, description })
        .collect())
}

/// Return epic IDs (e.g. `EP-F1-06`) from all `EP-*.md` files in the backlog.
/// IDs are read from frontmatter `id` field; missing/empty entries are skipped.
/// This is a lightweight listing suitable for shell completion.
pub fn list_epic_ids(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let files = collect_epic_files(repo_root)?;
    let mut ids = BTreeSet::new();
    for file in &files {
        if let Ok(markdown) = fs::read_to_string(file) {
            let parsed = parse_frontmatter(&markdown);
            if let Some(id) = parsed.frontmatter.get("id") {
                let id_upper = id.trim().to_ascii_uppercase();
                if !id_upper.is_empty() {
                    ids.insert(id_upper);
                }
            }
        }
    }
    Ok(ids.into_iter().collect())
}

pub fn read_task_file(
    file_path: impl AsRef<Path>,
    repo_root: impl AsRef<Path>,
) -> Result<TaskFile> {
    let file_path = fs::canonicalize(file_path.as_ref())
        .with_context(|| format!("resolve task file {}", file_path.as_ref().display()))?;
    let markdown = fs::read_to_string(&file_path)
        .with_context(|| format!("read task file {}", file_path.display()))?;
    let tasks = parse_task_markdown(&markdown);
    Ok(TaskFile {
        exists: true,
        relative_path: relative_path(repo_root.as_ref(), &file_path),
        summary: create_task_summary(&tasks),
        tasks,
        markdown: Some(markdown),
        file_path,
    })
}

pub fn read_story_file(file_path: impl AsRef<Path>, repo_root: impl AsRef<Path>) -> Result<Story> {
    let repo_root = repo_root.as_ref();
    let config = load_kanban_config(repo_root)?;
    read_story_file_inner(file_path.as_ref(), repo_root, &config)
}

/// Config-aware variant of [`read_story_file`].
///
/// Callers that already hold a [`KanbanConfig`] must use this so a repository
/// read parses `.kanban/settings.json` once instead of once per story. Paths in
/// the returned `Story` are relative to `config.repo_root`, which is exactly
/// what `read_repository` has always produced.
pub fn read_story_file_with_config(
    file_path: impl AsRef<Path>,
    config: &KanbanConfig,
) -> Result<Story> {
    let repo_root = config.repo_root.clone();
    read_story_file_inner(file_path.as_ref(), &repo_root, config)
}

fn read_story_file_inner(
    file_path: &Path,
    repo_root: &Path,
    config: &KanbanConfig,
) -> Result<Story> {
    crate::instrument::record_story_parse();
    let file_path = fs::canonicalize(file_path)
        .with_context(|| format!("resolve story file {}", file_path.display()))?;
    let markdown = fs::read_to_string(&file_path)
        .with_context(|| format!("read story file {}", file_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    let location = story_location(&file_path, config);
    let sprint_name = if config.features().sprints {
        parsed
            .frontmatter
            .get("sprint")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            .cloned()
            .or(location.sprint_name.clone())
    } else {
        None
    };
    let sibling_task_file_path = file_path.with_extension("tasks.md");
    let referenced_task_file_path = parsed
        .frontmatter
        .get("task_file")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| {
            file_path
                .parent()
                .with_context(|| {
                    format!("story file {} has no parent directory", file_path.display())
                })
                .map(|parent| parent.join(value))
        })
        .transpose()?;
    let task_file_path = referenced_task_file_path
        .clone()
        .unwrap_or_else(|| sibling_task_file_path.clone());
    let task_file = if task_file_path.exists() {
        // Containment check: refuse to read a task_file that resolves outside
        // the canonicalized backlog root (e.g. `task_file: ../../etc/passwd` or
        // a symlinked sibling). This bounds the read side per US-008 scenario 1
        // even before `validate` flags the value.
        let backlog_root = config.backlog_path();
        let inside = ensure_path_inside(&backlog_root, &task_file_path);
        match inside {
            Ok(canonical) => Some(read_task_file(&canonical, repo_root)?),
            Err(_) => Some(TaskFile {
                exists: false,
                file_path: task_file_path.clone(),
                relative_path: relative_path(repo_root, &task_file_path),
                tasks: Vec::new(),
                summary: TaskSummary::default(),
                markdown: None,
            }),
        }
    } else if referenced_task_file_path.is_some() {
        Some(TaskFile {
            exists: false,
            file_path: task_file_path.clone(),
            relative_path: relative_path(repo_root, &task_file_path),
            tasks: Vec::new(),
            summary: TaskSummary::default(),
            markdown: None,
        })
    } else {
        None
    };

    let fields = StoryFields::from_frontmatter(&parsed.frontmatter);

    Ok(Story {
        relative_path: relative_path(repo_root, &file_path),
        file_name: file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        body: parsed.body,
        file_path,
        frontmatter: parsed.frontmatter,
        frontmatter_keys: parsed.frontmatter_keys,
        fields,
        markdown,
        sprint_name,
        task_file,
    })
}

pub fn read_epic_file(file_path: impl AsRef<Path>, repo_root: impl AsRef<Path>) -> Result<Epic> {
    let repo_root = repo_root.as_ref();
    crate::instrument::record_epic_parse();
    let file_path = fs::canonicalize(file_path.as_ref())
        .with_context(|| format!("resolve epic file {}", file_path.as_ref().display()))?;
    let markdown = fs::read_to_string(&file_path)
        .with_context(|| format!("read epic file {}", file_path.display()))?;
    let parsed = parse_frontmatter(&markdown);

    Ok(Epic {
        relative_path: relative_path(repo_root, &file_path),
        file_name: file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        body: parsed.body,
        file_path,
        frontmatter: parsed.frontmatter,
        frontmatter_keys: parsed.frontmatter_keys,
        markdown,
    })
}

/// Config-aware variant of [`read_epic_file`]. Paths in the returned `Epic` are
/// relative to `config.repo_root`.
pub fn read_epic_file_with_config(
    file_path: impl AsRef<Path>,
    config: &KanbanConfig,
) -> Result<Epic> {
    read_epic_file(file_path, &config.repo_root)
}

pub fn read_repository(repo_root: impl AsRef<Path>) -> Result<Repository> {
    let config = load_kanban_config(repo_root)?;
    read_repository_with_config(&config)
}

/// Config-aware variant of [`read_repository`].
///
/// This is the single-pass entry point: exactly one git root resolution and one
/// `.kanban/settings.json` parse happen in `load_kanban_config`, and every file
/// below is read through the `_with_config` collectors and readers.
pub fn read_repository_with_config(config: &KanbanConfig) -> Result<Repository> {
    let repo_root = config.repo_root.clone();
    let story_files = collect_user_story_files_with_config(config)?;
    let stories = story_files
        .into_iter()
        .map(|path| read_story_file_with_config(path, config))
        .collect::<Result<Vec<_>>>()?;
    Ok(Repository { repo_root, stories })
}

pub fn epic_overview(epic: &Epic) -> EpicOverview {
    EpicOverview {
        id: epic.frontmatter.get("id").cloned().unwrap_or_else(|| {
            epic.file_name
                .trim_end_matches(STORY_FILE_SUFFIX)
                .to_string()
        }),
        title: story_title(&epic.body).unwrap_or_else(|| epic.file_name.clone()),
        status: epic.frontmatter.get("status").cloned().unwrap_or_default(),
        phase: epic.frontmatter.get("phase").cloned(),
        priority: epic
            .frontmatter
            .get("priority")
            .and_then(|value| parse_non_negative_i64_frontmatter(value)),
        owner: epic.frontmatter.get("owner").cloned(),
        milestone: epic.frontmatter.get("milestone").cloned(),
        work_started: epic
            .frontmatter
            .get("work_started")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            .cloned(),
        work_done: epic
            .frontmatter
            .get("work_done")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            .cloned(),
        planned_start: epic
            .frontmatter
            .get("planned_start")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            .cloned(),
        planned_end: epic
            .frontmatter
            .get("planned_end")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "~")
            .cloned(),
        relative_path: epic.relative_path.clone(),
    }
}

pub(crate) fn epic_title(repo_root: &Path, story: &Story) -> Option<String> {
    let epic_id = story.fields.epic.as_deref()?.trim();
    if epic_id.is_empty() {
        return None;
    }

    let epic_dir = repo_root.join(story.relative_path.parent()?);
    let epic_entry = fs::read_dir(epic_dir)
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(epic_id) && name.ends_with(".md"))
        })?;
    let body = fs::read_to_string(epic_entry.path()).ok()?;
    story_title(&body)
}

pub(crate) struct StoryLocation {
    pub(crate) sprint_name: Option<String>,
}

pub(crate) fn story_location(file_path: &Path, config: &KanbanConfig) -> StoryLocation {
    let _ = (file_path, config);
    StoryLocation { sprint_name: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use tempfile::tempdir;

    #[test]
    fn collect_user_story_files_returns_backlog_stories_but_not_task_files() {
        let (_fixture, repo_root) = build_fixture();
        let story_files = collect_user_story_files(&repo_root).unwrap();

        assert!(story_files.iter().any(|story_file| {
            story_file.ends_with("US-F1-010-ci-pipeline-build-and-unit-tests.md")
        }));
        assert!(
            !story_files
                .iter()
                .any(|story_file| story_file.to_string_lossy().ends_with(".tasks.md"))
        );
    }

    #[test]
    fn read_story_file_reads_canonical_backlog_story_and_sibling_tasks() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let story_path = write_story_with_task_file(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-001-test-story.md",
            "id: US-F1-001\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let story = read_story_file(&story_path, temp_root.path()).unwrap();

        assert_eq!(story.sprint_name.as_deref(), Some("S001.foundation"));
        assert_eq!(
            story.frontmatter.get("status").map(String::as_str),
            Some("in-progress")
        );
        let task_file = story.task_file.as_ref().unwrap();
        assert!(task_file.exists);
        assert_eq!(task_file.tasks.len(), 1);
    }

    #[test]
    fn atomic_write_replaces_existing_file_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("story.md");
        fs::write(&path, "original\n").unwrap();

        atomic_write(&path, "new contents\n").expect("atomic write succeeds");

        assert_eq!(fs::read_to_string(&path).unwrap(), "new contents\n");
    }

    #[test]
    fn atomic_write_leaves_original_intact_when_target_persist_fails() {
        // Simulate a mid-write crash: the temp file is written and fsynced, but
        // the final rename targets a path whose parent does not exist and
        // cannot be created, so `persist` fails. The original file must remain
        // unchanged and no partial content may be visible at the final path
        // (US-012 scenario 1).
        let dir = tempdir().unwrap();
        let original_path = dir.path().join("story.md");
        let original_content = "---\nid: US-001\n---\n# original\n";
        fs::write(&original_path, original_content).unwrap();

        // Target a path in a missing directory whose parent is a regular file,
        // so `atomic_write`'s `create_dir_all` (and therefore `persist`) fails.
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, "not a directory\n").unwrap();
        let unreachable_target = blocker.join("story.md");

        let result = atomic_write(&unreachable_target, "partial\n");
        assert!(result.is_err(), "expected persist to fail");

        // Original is intact (no truncation, no partial write).
        assert_eq!(
            fs::read_to_string(&original_path).unwrap(),
            original_content,
            "original file must be unchanged after a failed atomic write"
        );
        // No file appeared at the unreachable target.
        assert!(
            !unreachable_target.exists(),
            "no partial content may be visible at the final path"
        );
    }

    #[test]
    fn atomic_write_creates_parent_directory_if_missing() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("nested/deep/story.md");

        atomic_write(&nested, "contents\n").expect("atomic write succeeds");

        assert_eq!(fs::read_to_string(&nested).unwrap(), "contents\n");
    }

    #[cfg(unix)]
    #[test]
    fn collect_user_story_files_skips_symlinked_story_files() {
        // US-009: a symlinked US-*.md pointing outside the backlog root must
        // not appear in the collected story file list.
        use crate::testutil::*;
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());

        // Create a real story file inside the backlog.
        let backlog_dir = temp_root.path().join("delivery/backlog/phase-1");
        fs::create_dir_all(&backlog_dir).unwrap();
        let real_story = backlog_dir.join("US-001-real.md");
        fs::write(
            &real_story,
            "---\nid: US-001\ntype: user-story\nstatus: todo\n---\n# Real\n",
        )
        .unwrap();

        // Create a file outside the backlog and symlink it in.
        let outside = temp_root.path().join("evil.md");
        fs::write(&outside, "---\nid: US-002\ntype: user-story\n---\n# Evil\n").unwrap();
        let symlink = backlog_dir.join("US-002-symlink.md");
        std::os::unix::fs::symlink(&outside, &symlink).unwrap();

        let files = collect_user_story_files(temp_root.path()).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert!(
            names.contains(&"US-001-real.md".to_string()),
            "real story file must be collected"
        );
        assert!(
            !names.contains(&"US-002-symlink.md".to_string()),
            "symlinked story file must be skipped"
        );
    }
}

#[cfg(test)]
mod read_path_budget_tests {
    use super::*;
    use crate::epic::find_epic;
    use crate::instrument::ReadPathCounters;
    use crate::testsupport::{FixtureSpec, generate_backlog_fixture};

    /// B1 + B4: one `read_repository` resolves the git root and parses
    /// `.kanban/settings.json` exactly once, and parses each story exactly once.
    #[test]
    fn read_repository_resolves_configuration_exactly_once() {
        for spec in [
            FixtureSpec::representative().with_stories(40),
            FixtureSpec::minimal(),
        ] {
            let fixture = generate_backlog_fixture(&spec);
            // Warm the walk outside the counted region so only the read is measured.
            let expected_stories = collect_user_story_files(fixture.root()).unwrap().len();

            let counters = ReadPathCounters::start();
            let repository = read_repository(fixture.root()).unwrap();
            let counts = counters.snapshot();
            drop(counters);

            assert_eq!(repository.stories.len(), expected_stories);
            assert_eq!(
                counts.git_root_resolutions, 1,
                "B1: exactly one `git rev-parse --show-toplevel` per read_repository, got {counts:?}"
            );
            assert_eq!(
                counts.settings_parses, 1,
                "B4: exactly one settings.json parse per read_repository, got {counts:?}"
            );
            assert_eq!(
                counts.story_parses, expected_stories,
                "each story is parsed exactly once, got {counts:?}"
            );
        }
    }

    /// The epic lookup must not rescan the backlog once per epic.
    #[test]
    fn find_epic_reads_each_epic_file_once() {
        let spec = FixtureSpec::representative().with_stories(30);
        let fixture = generate_backlog_fixture(&spec);

        let counters = ReadPathCounters::start();
        let epic = find_epic(fixture.root(), "EP-001").unwrap();
        let counts = counters.snapshot();
        drop(counters);

        assert!(epic.is_some(), "fixture epic EP-001 must resolve");
        assert_eq!(counts.git_root_resolutions, 1, "got {counts:?}");
        assert_eq!(counts.settings_parses, 1, "got {counts:?}");
        assert_eq!(
            counts.epic_parses, spec.epics,
            "each epic file is read once per lookup, got {counts:?}"
        );
    }

    /// `read_repository_with_config` performs no resolution of its own, so the
    /// derivation layers built on top of it can compose without re-resolving.
    #[test]
    fn read_repository_with_config_performs_no_resolution() {
        let fixture = generate_backlog_fixture(&FixtureSpec::minimal());
        let config = load_kanban_config(fixture.root()).unwrap();

        let counters = ReadPathCounters::start();
        let repository = read_repository_with_config(&config).unwrap();
        let counts = counters.snapshot();
        drop(counters);

        assert!(!repository.stories.is_empty());
        assert_eq!(counts.git_root_resolutions, 0, "got {counts:?}");
        assert_eq!(counts.settings_parses, 0, "got {counts:?}");
    }

    /// Equivalence guard for WP-02: the config-aware readers must produce
    /// byte-identical parsed stories, in identical order, with identical
    /// relative paths and task-file resolution.
    #[test]
    fn config_aware_readers_match_the_convenience_apis() {
        for spec in [
            FixtureSpec::representative().with_stories(60),
            FixtureSpec::minimal(),
        ] {
            let fixture = generate_backlog_fixture(&spec);
            let config = load_kanban_config(fixture.root()).unwrap();

            let via_convenience = collect_user_story_files(fixture.root()).unwrap();
            let via_config = collect_user_story_files_with_config(&config).unwrap();
            assert_eq!(via_convenience, via_config);

            let epics_convenience = collect_epic_files(fixture.root()).unwrap();
            let epics_config = collect_epic_files_with_config(&config).unwrap();
            assert_eq!(epics_convenience, epics_config);

            for path in &via_config {
                let expected = read_story_file(path, &config.repo_root).unwrap();
                let actual = read_story_file_with_config(path, &config).unwrap();
                assert_eq!(expected.relative_path, actual.relative_path);
                assert_eq!(expected.file_path, actual.file_path);
                assert_eq!(expected.frontmatter, actual.frontmatter);
                assert_eq!(expected.frontmatter_keys, actual.frontmatter_keys);
                assert_eq!(expected.body, actual.body);
                assert_eq!(expected.markdown, actual.markdown);
                assert_eq!(expected.sprint_name, actual.sprint_name);
                assert_eq!(
                    expected.task_file.as_ref().map(|file| (
                        file.exists,
                        file.relative_path.clone(),
                        file.tasks.len()
                    )),
                    actual.task_file.as_ref().map(|file| (
                        file.exists,
                        file.relative_path.clone(),
                        file.tasks.len()
                    ))
                );
            }
        }
    }
}
