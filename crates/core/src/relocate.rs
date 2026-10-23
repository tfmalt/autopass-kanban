use crate::config::load_kanban_config;
use crate::epic::read_epic_sources;
use crate::error::KanbanError;
use crate::lock::RepoLock;
use crate::markdown::{story_title, upsert_frontmatter_markdown};
use crate::model::{Epic, RelocateStoryResult, Story};
use crate::prelude::*;
use crate::repository::{atomic_write, read_repository_with_config};
use crate::util::{
    current_timestamp_string, ensure_path_inside, relative_path, relative_path_from,
    to_forward_slashes,
};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};
use std::ops::Range;

const LINK_SEGMENT: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
    .remove(b'.')
    .remove(b'-')
    .remove(b'_')
    .remove(b'~');

pub fn relocate_story(
    repo_root: impl AsRef<Path>,
    story_id: &str,
    epic_id: &str,
) -> Result<RelocateStoryResult> {
    let config = load_kanban_config(repo_root)?;
    let _lock = RepoLock::acquire(&config.repo_root)?;
    if !config.features().epics {
        return Err(KanbanError::feature_disabled("epics").into());
    }
    let repository = read_repository_with_config(&config)?;
    let matching = repository
        .stories
        .iter()
        .filter(|story| story.fields.id.eq_ignore_ascii_case(story_id.trim()))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(KanbanError::invalid_argument(
            "Relocation requires exactly one canonical story with the requested id.",
        )
        .into());
    }
    let story = matching[0];
    let epics = read_epic_sources(&config)?;
    let resolve_epic = |id: &str| -> Result<&Epic> {
        let matches = epics
            .iter()
            .filter(|epic| {
                epic.frontmatter
                    .get("id")
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(id.trim()))
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(KanbanError::invalid_argument(format!(
                "Relocation requires exactly one canonical epic: {id}"
            ))
            .into());
        }
        Ok(matches[0])
    };
    let old_id = story
        .fields
        .epic
        .as_deref()
        .ok_or_else(|| KanbanError::invalid_argument("Story has no parent epic."))?;
    let source_epic = resolve_epic(old_id)?;
    let target_epic = resolve_epic(epic_id)?;
    let target_id = target_epic
        .frontmatter
        .get("id")
        .cloned()
        .ok_or_else(|| KanbanError::invalid_argument("Destination epic has no id."))?;
    let backlog = config.backlog_path();
    let source = ensure_path_inside(&backlog, &story.file_path)?;
    let directory = target_epic.file_path.parent().ok_or_else(|| {
        KanbanError::invalid_argument("Destination epic has no parent directory.")
    })?;
    let filename = source
        .file_name()
        .ok_or_else(|| KanbanError::invalid_argument("Story has no file name."))?;
    let target = ensure_path_inside(&backlog, &directory.join(filename))?;
    let mut result = RelocateStoryResult {
        story_id: story.fields.id.clone(),
        from_epic: old_id.to_string(),
        to_epic: target_id.clone(),
        from_path: relative_path(&config.repo_root, &source),
        story_path: relative_path(&config.repo_root, &target),
        task_path: story
            .task_file
            .as_ref()
            .filter(|task| task.exists)
            .map(|task| relative_path(&config.repo_root, &task.file_path)),
        changed_paths: Vec::new(),
    };
    if old_id.eq_ignore_ascii_case(&target_id) && source == target {
        return Ok(result);
    }
    let mut moves = BTreeMap::new();
    if source != target {
        moves.insert(source.clone(), target.clone());
    }
    let mut task_sources = BTreeSet::new();
    let sibling = source.with_extension("tasks.md");
    if sibling.exists() {
        task_sources.insert(ensure_path_inside(&backlog, &sibling)?);
    }
    if let Some(task) = &story.task_file {
        if !task.exists {
            return Err(KanbanError::invalid_argument(
                "Referenced task file is missing or unsafe.",
            )
            .into());
        }
        let path = ensure_path_inside(&backlog, &task.file_path)?;
        if repository.stories.iter().any(|other| {
            other.fields.id != story.fields.id
                && other
                    .task_file
                    .as_ref()
                    .is_some_and(|other_task| other_task.file_path == task.file_path)
        }) {
            return Err(KanbanError::invalid_argument(
                "Task file is shared by another story; relocation is unsafe.",
            )
            .into());
        }
        task_sources.insert(path);
    }
    for task_source in task_sources {
        let name = task_source
            .file_name()
            .ok_or_else(|| KanbanError::invalid_argument("Task file has no file name."))?;
        let task_target = ensure_path_inside(&backlog, &directory.join(name))?;
        if result
            .task_path
            .as_ref()
            .is_some_and(|path| config.repo_root.join(path) == task_source)
        {
            result.task_path = Some(relative_path(&config.repo_root, &task_target));
        }
        if task_source != task_target {
            moves.insert(task_source, task_target);
        }
    }
    for destination in moves.values() {
        if destination.exists() {
            return Err(KanbanError::invalid_argument(format!(
                "Destination already exists: {}",
                relative_path(&config.repo_root, destination).display()
            ))
            .into());
        }
    }
    let mut changes = BTreeMap::<PathBuf, Option<String>>::new();
    let mut originals = BTreeMap::<PathBuf, String>::new();
    for entry in WalkDir::new(&config.repo_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some(".git" | "target" | "node_modules" | "dist")
            )
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file()
            || entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "md")
        {
            continue;
        }
        let path = ensure_path_inside(&config.repo_root, entry.path())?;
        let original = fs::read_to_string(&path)?;
        originals.insert(path.clone(), original.clone());
        let destination = moves.get(&path).unwrap_or(&path);
        let mut updated = rewrite_links(&original, &path, destination, &moves)?;
        if path == source {
            updated = update_header_only(
                &updated,
                &[
                    ("epic", Some(target_id.clone())),
                    ("updated", Some(current_timestamp_string())),
                ],
            )?;
        }
        if path == source_epic.file_path {
            updated = update_children(&updated, story, false, &target)?;
        }
        if path == target_epic.file_path {
            updated = update_children(&updated, story, true, &target)?;
            if story.fields.status_raw == "in-progress"
                && target_epic
                    .frontmatter
                    .get("status")
                    .is_some_and(|status| matches!(status.as_str(), "draft" | "todo"))
            {
                updated =
                    update_header_only(&updated, &[("status", Some("in-progress".to_string()))])?;
            }
        }
        if destination != &path || updated != original {
            changes.insert(destination.clone(), Some(updated));
            if destination != &path {
                changes.insert(path, None);
            }
        }
    }
    for (task_source, task_target) in &moves {
        if task_source != &source && !changes.contains_key(task_target) {
            let text = fs::read_to_string(task_source)?;
            originals.insert(task_source.clone(), text.clone());
            changes.insert(task_target.clone(), Some(text));
            changes.insert(task_source.clone(), None);
        }
    }
    result.changed_paths = changes
        .keys()
        .map(|path| relative_path(&config.repo_root, path))
        .collect();
    let snapshots = changes
        .keys()
        .map(|path| (path.clone(), originals.get(path).cloned()))
        .collect();
    commit_changes_with(&changes, &snapshots, |path, content| {
        atomic_write(path, content)
    })?;
    Ok(result)
}

fn frontmatter_end(markdown: &str) -> Result<usize> {
    let mut offset = 0;
    let mut lines = markdown.split_inclusive('\n');
    if lines.next().is_none_or(|line| line.trim_end() != "---") {
        return Err(
            KanbanError::invalid_argument("Canonical Markdown requires frontmatter.").into(),
        );
    }
    offset += markdown
        .find('\n')
        .map_or(markdown.len(), |index| index + 1);
    for line in lines {
        offset += line.len();
        if line.trim_end() == "---" {
            return Ok(offset);
        }
    }
    Err(KanbanError::invalid_argument("Unterminated frontmatter.").into())
}

fn update_header_only(markdown: &str, updates: &[(&str, Option<String>)]) -> Result<String> {
    let end = frontmatter_end(markdown)?;
    let header = upsert_frontmatter_markdown(&markdown[..end], updates)?;
    let updated_end = frontmatter_end(&header)?;
    let header = if markdown[..end].contains("\r\n") {
        header[..updated_end].replace('\n', "\r\n")
    } else {
        header[..updated_end].to_string()
    };
    Ok(header + &markdown[end..])
}

fn update_children(markdown: &str, story: &Story, add: bool, target: &Path) -> Result<String> {
    let end = frontmatter_end(markdown)?;
    let header = &markdown[..end];
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(
        header
            .trim_start_matches("---")
            .trim_end()
            .trim_end_matches("---"),
    )?;
    let mut updated = markdown.to_string();
    if let Some(value) = yaml.get("user_stories") {
        let mut members: Vec<String> = if value.is_null() {
            Vec::new()
        } else {
            serde_yaml_ng::from_value(value.clone())?
        };
        members.retain(|id| !id.eq_ignore_ascii_case(&story.fields.id));
        if add {
            members.push(story.fields.id.clone());
        }
        let mut offset = 0;
        let mut field = None;
        for line in header.split_inclusive('\n') {
            if line.starts_with("user_stories:") {
                field = Some(offset..offset + line.len());
            } else if let Some(range) = &mut field {
                if line.starts_with([' ', '\t']) || line.trim().is_empty() {
                    range.end = offset + line.len();
                } else {
                    break;
                }
            }
            offset += line.len();
        }
        if let Some(field) = field {
            let newline = if header.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            updated.replace_range(
                field,
                &format!(
                    "user_stories: {}{newline}",
                    serde_json::to_string(&members)?
                ),
            );
        }
    }
    let section_pattern =
        Regex::new(r"(?m)^## (?:Child User Stories|Underliggende brukerhistorier)[^\r\n]*\r?\n")?;
    let section = section_pattern.find(&updated);
    if let Some(section) = section {
        let start = section.end();
        let end = Regex::new(r"(?m)^## ")?
            .find(&updated[start..])
            .map_or(updated.len(), |next| start + next.start());
        let mut body = String::new();
        let ids = Regex::new(r"US-(?:F[0-9]+-)?[0-9]+")?;
        for line in updated[start..end].split_inclusive('\n') {
            let first = line
                .trim_start()
                .strip_prefix('|')
                .and_then(|rest| rest.split('|').next())
                .unwrap_or_default();
            if ids
                .find(first)
                .is_some_and(|id| id.as_str() == story.fields.id)
            {
                continue;
            }
            body.push_str(line);
        }
        if add {
            let link = format!(
                "[{}]({})",
                story.fields.id,
                target.file_name().unwrap_or_default().to_string_lossy()
            );
            let title = story_title(&story.body)
                .unwrap_or_else(|| story.file_name.clone())
                .replace('|', "\\|");
            let row = if body
                .lines()
                .find(|line| line.trim_start().starts_with('|'))
                .is_some_and(|line| line.split('|').count() == 4)
            {
                format!("| {link} | {title} |\n")
            } else {
                format!(
                    "| {link} | {title} | {} | {} |\n",
                    story.fields.status_raw, story.fields.story_points_raw
                )
            };
            let mut lines = body
                .split_inclusive('\n')
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            if let Some(last) = lines
                .iter()
                .rposition(|line| line.trim_start().starts_with('|'))
            {
                lines.insert(last + 1, row);
            } else {
                lines.push(format!(
                    "\n| Story ID | Title | Status | Points |\n| --- | --- | --- | --- |\n{row}"
                ));
            }
            body = lines.concat();
        }
        updated.replace_range(start..end, &body);
    } else if add {
        updated.push_str(&format!("\n## Child User Stories\n\n| Story ID | Title | Status | Points |\n| --- | --- | --- | --- |\n| [{}]({}) | {} | {} | {} |\n", story.fields.id, target.file_name().unwrap_or_default().to_string_lossy(), story_title(&story.body).unwrap_or_else(|| story.file_name.clone()).replace('|', "\\|"), story.fields.status_raw, story.fields.story_points_raw));
    }
    Ok(updated)
}

fn rewrite_links(
    markdown: &str,
    source: &Path,
    destination: &Path,
    moves: &BTreeMap<PathBuf, PathBuf>,
) -> Result<String> {
    let parser = Parser::new_ext(markdown, Options::ENABLE_TABLES);
    let mut targets = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| (definition.dest.to_string(), definition.span.clone(), true))
        .collect::<Vec<_>>();
    for (event, range) in parser.into_offset_iter() {
        if let Event::Start(
            Tag::Link {
                dest_url,
                link_type: LinkType::Inline,
                ..
            }
            | Tag::Image {
                dest_url,
                link_type: LinkType::Inline,
                ..
            },
        ) = event
        {
            targets.push((dest_url.to_string(), range, false));
        }
    }
    let mut replacements = BTreeMap::<usize, (Range<usize>, String)>::new();
    for (url, range, definition) in targets {
        if url.is_empty() || url.starts_with(['#', '/']) || url.contains(':') {
            continue;
        }
        let (path, fragment) = url
            .split_once('#')
            .map_or((url.as_str(), None), |(path, fragment)| {
                (path, Some(fragment))
            });
        let decoded = percent_decode_str(path).decode_utf8()?;
        let resolved = match source
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(decoded.as_ref())
            .canonicalize()
        {
            Ok(path) => path,
            Err(_) => continue,
        };
        let target = moves.get(&resolved).unwrap_or(&resolved);
        if source == destination && !moves.contains_key(&resolved) {
            continue;
        }
        let relative = to_forward_slashes(&relative_path_from(
            destination.parent().unwrap_or_else(|| Path::new(".")),
            target,
        ));
        let encoded = relative
            .split('/')
            .map(|segment| utf8_percent_encode(segment, LINK_SEGMENT).to_string())
            .collect::<Vec<_>>()
            .join("/");
        let replacement =
            fragment.map_or(encoded.clone(), |fragment| format!("{encoded}#{fragment}"));
        let span = &markdown[range.clone()];
        let prefix = if definition {
            span.find("]:").map(|offset| offset + 2)
        } else {
            span.rfind("](").map(|offset| offset + 2)
        };
        if let Some(offset) =
            prefix.and_then(|prefix| span[prefix..].find(&url).map(|offset| prefix + offset))
        {
            let start = range.start + offset;
            replacements.insert(start, (start..start + url.len(), replacement));
        } else {
            return Err(KanbanError::invalid_argument(
                "Cannot safely rewrite an escaped Markdown destination.",
            )
            .into());
        }
    }
    let mut updated = markdown.to_string();
    for (_, (range, replacement)) in replacements.into_iter().rev() {
        updated.replace_range(range, &replacement);
    }
    Ok(updated)
}

fn commit_changes_with(
    changes: &BTreeMap<PathBuf, Option<String>>,
    snapshots: &BTreeMap<PathBuf, Option<String>>,
    mut write: impl FnMut(&Path, &str) -> Result<()>,
) -> Result<()> {
    for (path, expected) in snapshots {
        let current = if path.exists() {
            Some(fs::read_to_string(path)?)
        } else {
            None
        };
        if &current != expected {
            return Err(KanbanError::invalid_argument("A file changed while relocation was being planned; no changes applied. Retry after reviewing concurrent edits.").into());
        }
    }
    let result = (|| -> Result<()> {
        for (path, content) in changes {
            if let Some(content) = content {
                write(path, content)?;
            }
        }
        for (path, content) in changes {
            if content.is_none() {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        for (path, content) in snapshots {
            let restored = match content {
                Some(content) => atomic_write(path, content),
                None if path.exists() => fs::remove_file(path).map_err(Into::into),
                None => Ok(()),
            };
            if let Err(failure) = restored {
                failures.push(format!("{}: {failure}", path.display()));
            }
        }
        if !failures.is_empty() {
            bail!(
                "Relocation failed: {error}; rollback incomplete: {}",
                failures.join("; ")
            );
        }
        return Err(error.context("Relocation failed; original files restored"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::build_fixture;

    fn destination(root: &Path) -> PathBuf {
        let directory = root.join("delivery/backlog/phase-4-finalization/08.agreements");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("EP-F4-08-agreements.md"), "---\nid: EP-F4-08\ntype: epic\nstatus: draft\nphase: 4\nuser_stories: []\n---\n\n# Epic: Agreements\n\n## Child User Stories\n\n| Story ID | Title | Status | Points |\n| --- | --- | --- | --- |\n").unwrap();
        directory
    }

    #[test]
    fn relocation_preserves_scope_tasks_metadata_and_repairs_parent_and_links() {
        let (_fixture, root) = build_fixture();
        let directory = destination(&root);
        let repository = crate::read_repository(&root).unwrap();
        let story = repository
            .stories
            .iter()
            .find(|story| story.fields.id == "US-F1-052")
            .unwrap();
        let original = story.markdown.clone();
        let tasks = fs::read_to_string(&story.task_file.as_ref().unwrap().file_path).unwrap();
        fs::write(
            root.join("reference.md"),
            format!(
                "[story]({})\n\n[ref]: {}\n\n[reference][ref]\n\n```text\n[not a link]({})\n```\n",
                story.relative_path.display(),
                story.relative_path.display(),
                story.relative_path.display()
            ),
        )
        .unwrap();
        let result = relocate_story(&root, "US-F1-052", "EP-F4-08").unwrap();
        assert!(!story.file_path.exists());
        let moved = fs::read_to_string(root.join(&result.story_path)).unwrap();
        assert_eq!(
            &moved[frontmatter_end(&moved).unwrap()..],
            &original[frontmatter_end(&original).unwrap()..]
        );
        let metadata = crate::parse_frontmatter(&moved).frontmatter;
        for (key, value) in &story.frontmatter {
            if !matches!(key.as_str(), "epic" | "updated") {
                assert_eq!(metadata.get(key), Some(value), "{key}");
            }
        }
        assert_eq!(metadata.get("epic").unwrap(), "EP-F4-08");
        assert_eq!(
            fs::read_to_string(root.join(result.task_path.unwrap())).unwrap(),
            tasks
        );
        let parent = fs::read_to_string(directory.join("EP-F4-08-agreements.md")).unwrap();
        assert!(parent.contains("user_stories: [\"US-F1-052\"]"));
        assert!(parent.contains("| [US-F1-052]"));
        let reference = fs::read_to_string(root.join("reference.md")).unwrap();
        assert!(reference.contains(&format!("[story]({})", result.story_path.display())));
        assert!(reference.contains(&format!("[ref]: {}", result.story_path.display())));
        assert!(reference.contains(&format!("[not a link]({})", story.relative_path.display())));
        assert!(
            relocate_story(&root, "US-F1-052", "EP-F4-08")
                .unwrap()
                .changed_paths
                .is_empty()
        );
    }

    #[test]
    fn relocation_rejects_collisions_before_writing() {
        let (_fixture, root) = build_fixture();
        let directory = destination(&root);
        let source = crate::read_repository(&root)
            .unwrap()
            .stories
            .into_iter()
            .find(|story| story.fields.id == "US-F1-052")
            .unwrap();
        let before = fs::read(&source.file_path).unwrap();
        fs::write(
            directory.join(source.file_path.file_name().unwrap()),
            "do not replace",
        )
        .unwrap();
        assert!(relocate_story(&root, "US-F1-052", "EP-F4-08").is_err());
        assert_eq!(fs::read(&source.file_path).unwrap(), before);
        assert_eq!(
            fs::read_to_string(directory.join(source.file_path.file_name().unwrap())).unwrap(),
            "do not replace"
        );
    }

    #[test]
    fn relocation_rebases_outgoing_links_and_keeps_custom_task_filename() {
        let (_fixture, root) = build_fixture();
        destination(&root);
        let source = crate::read_repository(&root)
            .unwrap()
            .stories
            .into_iter()
            .find(|story| story.fields.id == "US-F1-052")
            .unwrap();
        let task = source.task_file.unwrap().file_path;
        let custom = task.parent().unwrap().join("custom.tasks.md");
        fs::rename(&task, &custom).unwrap();
        let text = upsert_frontmatter_markdown(
            &source.markdown,
            &[("task_file", Some("custom.tasks.md".into()))],
        )
        .unwrap();
        fs::write(
            &source.file_path,
            format!("{text}\n[epic](EP-F1-06-git-driven-kanban-and-backlog-tooling.md#scope)\n"),
        )
        .unwrap();
        let result = relocate_story(&root, "US-F1-052", "EP-F4-08").unwrap();
        assert!(result.task_path.unwrap().ends_with("custom.tasks.md"));
        let moved = fs::read_to_string(root.join(result.story_path)).unwrap();
        assert!(moved.contains("[epic](../../phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/EP-F1-06-git-driven-kanban-and-backlog-tooling.md#scope)"));
        assert!(moved.contains("task_file: custom.tasks.md"));
    }

    #[test]
    fn relocation_preserves_crlf_story_body_and_block_parent_membership() {
        let (_fixture, root) = build_fixture();
        let directory = destination(&root);
        let source = crate::read_repository(&root)
            .unwrap()
            .stories
            .into_iter()
            .find(|story| story.fields.id == "US-F1-052")
            .unwrap();
        fs::write(&source.file_path, source.markdown.replace('\n', "\r\n")).unwrap();
        let parent = fs::read_to_string(&source.file_path).unwrap();
        let target_epic = directory.join("EP-F4-08-agreements.md");
        let text = fs::read_to_string(&target_epic)
            .unwrap()
            .replace("user_stories: []", "user_stories:\n  - US-F4-999");
        fs::write(&target_epic, text).unwrap();
        let result = relocate_story(&root, "US-F1-052", "EP-F4-08").unwrap();
        let moved = fs::read_to_string(root.join(result.story_path)).unwrap();
        assert_eq!(
            &moved[frontmatter_end(&moved).unwrap()..],
            &parent[frontmatter_end(&parent).unwrap()..]
        );
        let updated = fs::read_to_string(target_epic).unwrap();
        assert!(updated.contains("user_stories: [\"US-F4-999\",\"US-F1-052\"]"));
    }

    #[test]
    fn relocation_write_failure_restores_all_originals() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.md");
        let second = directory.path().join("second.md");
        fs::write(&first, "original").unwrap();
        let changes = BTreeMap::from([
            (first.clone(), Some("changed".into())),
            (second.clone(), Some("new".into())),
        ]);
        let snapshots = BTreeMap::from([
            (first.clone(), Some("original".into())),
            (second.clone(), None),
        ]);
        let mut count = 0;
        let error = commit_changes_with(&changes, &snapshots, |path, content| {
            count += 1;
            if count == 2 {
                bail!("injected write failure");
            }
            atomic_write(path, content)
        })
        .unwrap_err();
        assert!(error.to_string().contains("original files restored"));
        assert_eq!(fs::read_to_string(first).unwrap(), "original");
        assert!(!second.exists());
    }

    #[test]
    fn relocation_rejects_concurrent_edit_without_overwriting_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("story.md");
        fs::write(&path, "editor change").unwrap();
        let changes = BTreeMap::from([(path.clone(), Some("planned".into()))]);
        let snapshots = BTreeMap::from([(path.clone(), Some("before".into()))]);
        assert!(
            commit_changes_with(&changes, &snapshots, |path, content| atomic_write(
                path, content
            ))
            .is_err()
        );
        assert_eq!(fs::read_to_string(path).unwrap(), "editor change");
    }

    #[test]
    fn relocation_does_not_rewrite_reference_labels_or_link_titles() {
        let (_fixture, root) = build_fixture();
        destination(&root);
        let source = crate::read_repository(&root)
            .unwrap()
            .stories
            .into_iter()
            .find(|story| story.fields.id == "US-F1-052")
            .unwrap();
        let old = to_forward_slashes(&source.relative_path);
        fs::write(
            root.join("references.md"),
            format!("[{old}][ref]\n\n[ref]: {old} \"{old}\"\n\n[inline]({old} \"{old}\")\n"),
        )
        .unwrap();
        let result = relocate_story(&root, "US-F1-052", "EP-F4-08").unwrap();
        let new = to_forward_slashes(&result.story_path);
        let text = fs::read_to_string(root.join("references.md")).unwrap();
        assert!(text.contains(&format!("[{old}][ref]")));
        assert!(text.contains(&format!("[ref]: {new} \"{old}\"")));
        assert!(text.contains(&format!("[inline]({new} \"{old}\")")));
    }

    #[cfg(unix)]
    #[test]
    fn relocation_rejects_symlinked_task_escape_without_writes() {
        let (_fixture, root) = build_fixture();
        destination(&root);
        let source = crate::read_repository(&root)
            .unwrap()
            .stories
            .into_iter()
            .find(|story| story.fields.id == "US-F1-052")
            .unwrap();
        let task = source.task_file.unwrap().file_path;
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("outside.md");
        fs::write(&secret, "do not touch").unwrap();
        fs::remove_file(&task).unwrap();
        std::os::unix::fs::symlink(&secret, &task).unwrap();
        assert!(relocate_story(&root, "US-F1-052", "EP-F4-08").is_err());
        assert_eq!(
            fs::read_to_string(source.file_path).unwrap(),
            source.markdown
        );
        assert_eq!(fs::read_to_string(secret).unwrap(), "do not touch");
    }
}
