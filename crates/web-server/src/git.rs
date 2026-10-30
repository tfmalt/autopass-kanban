use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use kanban_core::{ROSTER_HEADING, atomic_write, load_kanban_config};

use crate::pending::{self, PendingChange, PendingStore};

#[derive(Debug, Clone)]
pub(crate) struct GitContext {
    pub(crate) repo_root: PathBuf,
    pub(crate) git_dir: PathBuf,
}

impl GitContext {
    pub(crate) fn discover(repo_root: &Path) -> Option<Self> {
        let output = Command::new("git")
            .args([
                "-C",
                &repo_root.to_string_lossy(),
                "rev-parse",
                "--absolute-git-dir",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let git_dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        (!git_dir.as_os_str().is_empty()).then_some(Self {
            repo_root: repo_root.to_path_buf(),
            git_dir,
        })
    }

    pub(crate) fn pending_file(&self) -> PathBuf {
        self.git_dir.join("kanban").join("pending-web-changes.json")
    }
}

pub(crate) fn run(repo_root: &Path, args: &[&str], network: bool) -> std::io::Result<Output> {
    let mut command = Command::new("git");
    command
        .args(["-C", &repo_root.to_string_lossy()])
        .args(args);
    if network {
        command.env("GIT_TERMINAL_PROMPT", "0");
    }
    command.output()
}

pub(crate) fn run_owned(
    repo_root: &Path,
    args: &[String],
    network: bool,
) -> std::io::Result<Output> {
    let mut command = Command::new("git");
    command
        .args(["-C", &repo_root.to_string_lossy()])
        .args(args);
    if network {
        command.env("GIT_TERMINAL_PROMPT", "0");
    }
    command.output()
}

pub(crate) fn output_text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .trim()
    .to_string()
}

pub(crate) fn branch(repo_root: &Path) -> String {
    run(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"], false)
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Debug, Default)]
pub(crate) struct UpstreamState {
    pub(crate) upstream: Option<String>,
    pub(crate) ahead: u32,
    pub(crate) behind: u32,
}

#[derive(Debug)]
pub(crate) struct PullOutcome {
    pub(crate) output: Output,
    pub(crate) recovered_paths: Vec<String>,
    pub(crate) rebased_local_changes: bool,
}

#[derive(Debug, Clone)]
struct WorktreeChange {
    path: PathBuf,
}

pub(crate) fn abort_in_progress_git_ops(git_context: &GitContext) {
    if git_context.git_dir.join("rebase-merge").exists()
        || git_context.git_dir.join("rebase-apply").exists()
    {
        let _ = run(&git_context.repo_root, &["rebase", "--abort"], false);
    }
    if git_context.git_dir.join("MERGE_HEAD").exists() {
        let _ = run(&git_context.repo_root, &["merge", "--abort"], false);
    }
    if git_context.git_dir.join("CHERRY_PICK_HEAD").exists() {
        let _ = run(&git_context.repo_root, &["cherry-pick", "--abort"], false);
    }
}

fn is_runtime_lock_path(path: &Path) -> bool {
    path == Path::new(".kanban/.lock") || path.starts_with(".kanban/run")
}

fn managed_prefixes(repo_root: &Path) -> Vec<PathBuf> {
    let mut prefixes = vec![
        PathBuf::from("delivery/backlog"),
        PathBuf::from("delivery/sprints"),
        PathBuf::from(".kanban"),
    ];
    if let Ok(config) = load_kanban_config(repo_root) {
        if let Ok(rel) = config.backlog_path().strip_prefix(&config.repo_root)
            && !rel.as_os_str().is_empty()
        {
            prefixes.push(rel.to_path_buf());
        }
        if let Ok(rel) = config.sprints_path().strip_prefix(&config.repo_root)
            && !rel.as_os_str().is_empty()
        {
            prefixes.push(rel.to_path_buf());
        }
    }
    prefixes
}

pub(crate) fn is_managed_kanban_path(repo_root: &Path, path: &Path) -> bool {
    if is_runtime_lock_path(path)
        || path.is_absolute()
        || path.to_string_lossy().starts_with('-')
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }
    managed_prefixes(repo_root)
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

pub(crate) fn effective_pending_count(
    repo_root: &Path,
    sprint_paths: &[PathBuf],
    recorded: &[PendingChange],
) -> std::io::Result<usize> {
    let changes = worktree_changes(repo_root)?;
    let recorded_paths = pending::commit_paths(recorded)
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect::<BTreeSet<_>>();
    let mut managed_dirty = 0usize;
    let mut non_generated_dirty = 0usize;
    for change in &changes {
        if is_managed_kanban_path(repo_root, &change.path) || recorded_paths.contains(&change.path)
        {
            managed_dirty += 1;
            let generated_only = sprint_paths.contains(&change.path)
                && generated_roster_is_only_change(repo_root, &change.path).unwrap_or(false);
            if !generated_only {
                non_generated_dirty += 1;
            }
        }
    }
    if managed_dirty == 0 {
        return Ok(0);
    }
    if !recorded.is_empty() {
        Ok(recorded.len())
    } else {
        Ok(non_generated_dirty)
    }
}

struct PreparedRosterState {
    snapshots: Vec<(PathBuf, String)>,
    recovered_paths: Vec<String>,
}

fn reset_generated_roster_sections_to_head(
    repo_root: &Path,
    sprint_paths: &[PathBuf],
) -> std::io::Result<PreparedRosterState> {
    let changes = worktree_changes(repo_root)?;
    let mut snapshots = Vec::new();
    let mut recovered_paths = Vec::new();
    let mut restore_to_head = Vec::new();

    for change in &changes {
        if !sprint_paths.contains(&change.path) {
            continue;
        }
        let full_path = repo_root.join(&change.path);
        let Ok(current) = fs::read_to_string(&full_path) else {
            continue;
        };
        let Some(head) = head_file_content(repo_root, &change.path)? else {
            continue;
        };
        if current == head {
            continue;
        }
        let Some(current_roster) = generated_roster_offset(&current) else {
            continue;
        };
        let Some(head_roster) = generated_roster_offset(&head) else {
            continue;
        };
        snapshots.push((change.path.clone(), current.clone()));
        if current[..current_roster] == head[..head_roster] {
            restore_to_head.push(change.path.to_string_lossy().into_owned());
            recovered_paths.push(change.path.to_string_lossy().into_owned());
        } else if current[current_roster..] != head[head_roster..] {
            let merged_pre_roster =
                format!("{}{}", &current[..current_roster], &head[head_roster..]);
            atomic_write(&full_path, &merged_pre_roster)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }
    }

    if !restore_to_head.is_empty() {
        let mut reset_args = vec![
            "reset".to_string(),
            "--quiet".to_string(),
            "HEAD".to_string(),
            "--".to_string(),
        ];
        reset_args.extend(restore_to_head.iter().cloned());
        let _ = run_owned(repo_root, &reset_args, false);

        let mut restore_args = vec![
            "restore".to_string(),
            "--worktree".to_string(),
            "--".to_string(),
        ];
        restore_args.extend(restore_to_head);
        let restore = run_owned(repo_root, &restore_args, false)?;
        if !restore.status.success() {
            restore_snapshot_files(repo_root, &snapshots)?;
            return Err(std::io::Error::other(format!(
                "could not safely prepare generated sprint rosters: {}",
                output_text(&restore)
            )));
        }
    }

    recovered_paths.sort();
    recovered_paths.dedup();
    Ok(PreparedRosterState {
        snapshots,
        recovered_paths,
    })
}

pub(crate) fn commit_pending_kanban_changes(
    repo_root: &Path,
    sprint_paths: &[PathBuf],
    pending_store: Option<&PendingStore>,
    strip_generated_rosters_first: bool,
) -> std::io::Result<Option<String>> {
    if strip_generated_rosters_first {
        let _ = reset_generated_roster_sections_to_head(repo_root, sprint_paths)?;
    }
    let recorded = match pending_store {
        Some(store) => store
            .load()
            .map_err(|error| std::io::Error::other(error.to_string()))?,
        None => Vec::new(),
    };
    let recorded_paths = pending::commit_paths(&recorded)
        .map_err(|error| std::io::Error::other(error.to_string()))?
        .into_iter()
        .map(PathBuf::from)
        .collect::<BTreeSet<_>>();

    let changes = worktree_changes(repo_root)?;
    let mut candidate_paths = BTreeSet::new();
    let mut has_non_generated = !recorded.is_empty();
    for change in &changes {
        if is_managed_kanban_path(repo_root, &change.path) || recorded_paths.contains(&change.path)
        {
            let generated_only = sprint_paths.contains(&change.path)
                && generated_roster_is_only_change(repo_root, &change.path).unwrap_or(false);
            if !generated_only {
                has_non_generated = true;
            }
            candidate_paths.insert(change.path.to_string_lossy().into_owned());
        }
    }

    if candidate_paths.is_empty() || !has_non_generated {
        if !recorded.is_empty()
            && candidate_paths.is_empty()
            && let Some(store) = pending_store
        {
            store
                .clear()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }
        return Ok(None);
    }

    let paths_vec = candidate_paths.into_iter().collect::<Vec<_>>();
    let mut add_args = vec!["add".to_string(), "-A".to_string(), "--".to_string()];
    add_args.extend(paths_vec.iter().cloned());
    let add_output = run_owned(repo_root, &add_args, false)?;
    if !add_output.status.success() {
        return Err(std::io::Error::other(format!(
            "Commit preparation failed: {}",
            output_text(&add_output)
        )));
    }

    let message = pending::commit_message_or_paths(&recorded, &paths_vec);
    let mut commit_args = vec![
        "commit".to_string(),
        "--only".to_string(),
        "-m".to_string(),
        message,
        "--".to_string(),
    ];
    commit_args.extend(paths_vec);
    let commit_output = run_owned(repo_root, &commit_args, false)?;
    if !commit_output.status.success() {
        return Err(std::io::Error::other(format!(
            "Commit failed: {}",
            output_text(&commit_output)
                .chars()
                .take(200)
                .collect::<String>()
        )));
    }

    if let Some(store) = pending_store {
        store
            .clear()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
    }

    head_sha(repo_root).map(Some)
}

pub(crate) fn amend_head_with_sprint_rosters(
    repo_root: &Path,
    sprint_paths: &[PathBuf],
) -> std::io::Result<Option<String>> {
    if upstream_state(repo_root).ahead == 0 || sprint_paths.is_empty() {
        return head_sha(repo_root).map(Some);
    }
    let changes = worktree_changes(repo_root)?;
    let dirty_sprints = changes
        .iter()
        .filter(|change| sprint_paths.contains(&change.path))
        .map(|change| change.path.to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if dirty_sprints.is_empty() {
        return head_sha(repo_root).map(Some);
    }

    let _ = run(repo_root, &["reset", "--quiet", "HEAD", "--", "."], false);
    let mut add_args = vec!["add".to_string(), "-A".to_string(), "--".to_string()];
    add_args.extend(dirty_sprints);
    let add_out = run_owned(repo_root, &add_args, false)?;
    if !add_out.status.success() {
        return Err(std::io::Error::other(output_text(&add_out)));
    }
    let staged = run(repo_root, &["diff", "--cached", "--quiet"], false)?;
    if !staged.status.success() {
        let amend = run(repo_root, &["commit", "--amend", "--no-edit"], false)?;
        if !amend.status.success() {
            return Err(std::io::Error::other(output_text(&amend)));
        }
    }
    head_sha(repo_root).map(Some)
}

pub(crate) fn head_sha(repo_root: &Path) -> std::io::Result<String> {
    let head = run(repo_root, &["rev-parse", "HEAD"], false)?;
    if !head.status.success() {
        return Err(std::io::Error::other(output_text(&head)));
    }
    Ok(String::from_utf8_lossy(&head.stdout).trim().to_string())
}

fn rebase_in_progress(git_dir: &Path) -> bool {
    git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists()
}

fn unmerged_paths(repo_root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let output = run(
        repo_root,
        &["diff", "--name-only", "--diff-filter=U", "-z"],
        false,
    )?;
    if !output.status.success() {
        return Err(std::io::Error::other(output_text(&output)));
    }
    let mut paths = Vec::new();
    for entry in output.stdout.split(|byte| *byte == 0) {
        if !entry.is_empty() {
            paths.push(PathBuf::from(status_utf8(entry)?));
        }
    }
    Ok(paths)
}

pub(crate) fn rebase_onto_upstream(git_context: &GitContext) -> std::io::Result<Output> {
    let mut output = run(
        &git_context.repo_root,
        &["rebase", "--autostash", "-X", "theirs", "@{upstream}"],
        false,
    )?;
    let mut steps = 0usize;
    while !output.status.success() && rebase_in_progress(&git_context.git_dir) && steps < 32 {
        steps += 1;
        let conflicted = unmerged_paths(&git_context.repo_root).unwrap_or_default();
        if conflicted.is_empty()
            || !conflicted
                .iter()
                .all(|path| is_managed_kanban_path(&git_context.repo_root, path))
        {
            let _ = run(&git_context.repo_root, &["rebase", "--abort"], false);
            return Ok(output);
        }
        for path in &conflicted {
            let path_str = path.to_string_lossy().into_owned();
            let checkout = run_owned(
                &git_context.repo_root,
                &[
                    "checkout".to_string(),
                    "--theirs".to_string(),
                    "--".to_string(),
                    path_str.clone(),
                ],
                false,
            )?;
            if checkout.status.success() {
                let _ = run_owned(
                    &git_context.repo_root,
                    &["add".to_string(), "--".to_string(), path_str],
                    false,
                );
            } else {
                let _ = run_owned(
                    &git_context.repo_root,
                    &["rm".to_string(), "--".to_string(), path_str],
                    false,
                );
            }
        }
        output = run(
            &git_context.repo_root,
            &["-c", "core.editor=true", "rebase", "--continue"],
            false,
        )?;
    }
    if !output.status.success() && rebase_in_progress(&git_context.git_dir) {
        let _ = run(&git_context.repo_root, &["rebase", "--abort"], false);
    }
    Ok(output)
}

pub(crate) fn sync_pull(
    git_context: &GitContext,
    sprint_paths: &[PathBuf],
    pending_store: Option<&PendingStore>,
) -> std::io::Result<PullOutcome> {
    let repo_root = &git_context.repo_root;
    abort_in_progress_git_ops(git_context);

    let prepared = reset_generated_roster_sections_to_head(repo_root, sprint_paths)?;
    let committed_sha =
        match commit_pending_kanban_changes(repo_root, sprint_paths, pending_store, false) {
            Ok(sha) => sha,
            Err(error) => {
                restore_snapshot_files(repo_root, &prepared.snapshots)?;
                return Err(error);
            }
        };

    let upstream_before = upstream_state(repo_root);
    if upstream_before.ahead == 0 && committed_sha.is_none() {
        let output = match run(repo_root, &["pull", "--ff-only"], true) {
            Ok(output) => output,
            Err(error) => {
                restore_snapshot_files(repo_root, &prepared.snapshots)?;
                return Err(error);
            }
        };
        if !output.status.success() {
            restore_snapshot_files(repo_root, &prepared.snapshots)?;
            return Ok(PullOutcome {
                output,
                recovered_paths: Vec::new(),
                rebased_local_changes: false,
            });
        }
        return Ok(PullOutcome {
            output,
            recovered_paths: prepared.recovered_paths,
            rebased_local_changes: false,
        });
    }

    let fetch = run(repo_root, &["fetch", "--quiet", "--no-tags"], true)?;
    if !fetch.status.success() {
        return Ok(PullOutcome {
            output: fetch,
            recovered_paths: Vec::new(),
            rebased_local_changes: false,
        });
    }

    let upstream = upstream_state(repo_root);
    if upstream.upstream.is_none() {
        let output = run(repo_root, &["pull", "--ff-only"], true)?;
        return Ok(PullOutcome {
            output,
            recovered_paths: Vec::new(),
            rebased_local_changes: false,
        });
    }

    if upstream.behind == 0 {
        return Ok(PullOutcome {
            output: fetch,
            recovered_paths: prepared.recovered_paths,
            rebased_local_changes: committed_sha.is_some(),
        });
    }

    let rebase_output = rebase_onto_upstream(git_context)?;
    if !rebase_output.status.success() {
        return Ok(PullOutcome {
            output: rebase_output,
            recovered_paths: Vec::new(),
            rebased_local_changes: false,
        });
    }

    Ok(PullOutcome {
        output: rebase_output,
        recovered_paths: prepared.recovered_paths,
        rebased_local_changes: true,
    })
}

fn worktree_changes(repo_root: &Path) -> std::io::Result<Vec<WorktreeChange>> {
    let output = run(
        repo_root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        false,
    )?;
    if !output.status.success() {
        return Err(std::io::Error::other(output_text(&output)));
    }

    let mut changes = Vec::new();
    let mut records = output.stdout.split(|byte| *byte == 0).peekable();
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        if record.len() < 4 || record[2] != b' ' {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "git returned an invalid porcelain status record",
            ));
        }
        let status = status_utf8(&record[..2])?.to_string();
        let path = PathBuf::from(status_utf8(&record[3..])?);
        if !is_runtime_lock_path(&path) {
            changes.push(WorktreeChange { path });
        }

        if (status.contains('R') || status.contains('C'))
            && let Some(original_path) = records.next().filter(|path| !path.is_empty())
        {
            let original = PathBuf::from(status_utf8(original_path)?);
            if !is_runtime_lock_path(&original) {
                changes.push(WorktreeChange { path: original });
            }
        }
    }
    Ok(changes)
}

fn status_utf8(bytes: &[u8]) -> std::io::Result<&str> {
    std::str::from_utf8(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn generated_roster_offset(markdown: &str) -> Option<usize> {
    markdown
        .match_indices(ROSTER_HEADING)
        .find_map(|(offset, _)| {
            let starts_line = offset == 0 || markdown.as_bytes().get(offset - 1) == Some(&b'\n');
            let end = offset + ROSTER_HEADING.len();
            let ends_line = end == markdown.len()
                || markdown[end..].starts_with('\n')
                || markdown[end..].starts_with("\r\n");
            (starts_line && ends_line).then_some(offset)
        })
}

fn head_file_content(repo_root: &Path, path: &Path) -> std::io::Result<Option<String>> {
    if path.is_absolute()
        || path.to_string_lossy().starts_with('-')
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Ok(None);
    }
    let spec = format!("HEAD:{}", path.to_string_lossy());
    let head = run_owned(repo_root, &["show".to_string(), spec], false)?;
    if !head.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8(head.stdout).ok())
}

fn generated_roster_is_only_change(repo_root: &Path, path: &Path) -> std::io::Result<bool> {
    if path.is_absolute()
        || path.to_string_lossy().starts_with('-')
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Ok(false);
    }
    let full_path = repo_root.join(path);
    let metadata = match fs::symlink_metadata(&full_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(false);
    }

    let Some(head) = head_file_content(repo_root, path)? else {
        return Ok(false);
    };
    let current = fs::read_to_string(full_path)?;
    if current == head {
        return Ok(false);
    }
    let Some(current_roster) = generated_roster_offset(&current) else {
        return Ok(false);
    };
    let Some(head_roster) = generated_roster_offset(&head) else {
        return Ok(false);
    };
    Ok(current[..current_roster] == head[..head_roster])
}

fn restore_snapshot_files(
    repo_root: &Path,
    snapshots: &[(PathBuf, String)],
) -> std::io::Result<()> {
    for (path, content) in snapshots {
        atomic_write(repo_root.join(path), content)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn upstream_state(repo_root: &Path) -> UpstreamState {
    let upstream = run(
        repo_root,
        &["rev-parse", "--abbrev-ref", "@{upstream}"],
        false,
    )
    .ok()
    .filter(|output| output.status.success())
    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    .filter(|value| !value.is_empty());
    let Some(upstream) = upstream else {
        return UpstreamState::default();
    };
    let counts = run(
        repo_root,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
        false,
    )
    .ok()
    .filter(|output| output.status.success())
    .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
    .unwrap_or_default();
    let mut values = counts
        .split_whitespace()
        .filter_map(|value| value.parse::<u32>().ok());
    UpstreamState {
        upstream: Some(upstream),
        behind: values.next().unwrap_or(0),
        ahead: values.next().unwrap_or(0),
    }
}

pub(crate) fn classify_pull_error(output: &str) -> String {
    let lower = output.to_lowercase();
    if lower.contains("conflict") {
        "Pull failed: merge conflict. Resolve conflicts locally before syncing.".to_string()
    } else if lower.contains("local changes") || lower.contains("would be overwritten") {
        "Pull failed: local uncommitted changes would be overwritten. Commit or stash them first."
            .to_string()
    } else if lower.contains("authentication")
        || lower.contains("auth")
        || lower.contains("403")
        || lower.contains("401")
    {
        "Pull failed: authentication error. Check your credentials.".to_string()
    } else if lower.contains("could not resolve host")
        || lower.contains("network")
        || lower.contains("unable to connect")
    {
        "Pull failed: network error. Check your internet connection.".to_string()
    } else if lower.contains("not a git repository") {
        "Pull failed: the data directory is not a git repository.".to_string()
    } else if lower.contains("no remote")
        || lower.contains("no tracking")
        || lower.contains("no upstream")
    {
        "Pull failed: no remote tracking branch configured.".to_string()
    } else if output.is_empty() {
        "git pull failed with no output.".to_string()
    } else {
        format!(
            "git pull failed: {}",
            output.chars().take(200).collect::<String>()
        )
    }
}

pub(crate) fn classify_push_error(output: &str) -> String {
    let lower = output.to_lowercase();
    if lower.contains("non-fast-forward") || lower.contains("fetch first") {
        "Push rejected: the remote has commits you do not have. Pull first, then push.".to_string()
    } else if lower.contains("authentication")
        || lower.contains("auth")
        || lower.contains("403")
        || lower.contains("401")
    {
        "Push failed: authentication error. Check your credentials.".to_string()
    } else if lower.contains("could not resolve host")
        || lower.contains("network")
        || lower.contains("unable to connect")
    {
        "Push failed: network error. Check your internet connection.".to_string()
    } else if output.is_empty() {
        "git push failed with no output.".to_string()
    } else {
        format!(
            "git push failed: {}",
            output.chars().take(200).collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::{TempDir, tempdir};

    const SPRINT_PATH: &str = "delivery/sprints/S001.generated.md";

    struct GitFixture {
        _local_dir: TempDir,
        _bare_dir: TempDir,
        _upstream_dir: TempDir,
        local: PathBuf,
        bare: PathBuf,
        upstream: PathBuf,
    }

    fn sprint_markdown(roster: &str, note: &str) -> String {
        format!(
            "---\nsprint: S001\n---\n\n# S001: generated\n\n{note}\n\n{ROSTER_HEADING}\n\n{roster}\n"
        )
    }

    fn git_ok(repo_root: &Path, args: &[&str]) -> Output {
        let output = run(repo_root, args, false).expect("git command should run");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            output_text(&output)
        );
        output
    }

    fn git_fixture() -> GitFixture {
        let local_dir = tempdir().expect("local repository temp dir");
        let bare_dir = tempdir().expect("bare repository temp dir");
        let upstream_dir = tempdir().expect("upstream checkout temp dir");
        let local = local_dir.path().to_path_buf();
        let bare = bare_dir.path().to_path_buf();
        let upstream = upstream_dir.path().to_path_buf();

        fs::create_dir_all(local.join("delivery/sprints")).expect("create sprint directory");
        fs::write(
            local.join(SPRINT_PATH),
            sprint_markdown("Generated roster v1", "Keep this note."),
        )
        .expect("write initial sprint file");
        git_ok(&local, &["init", "--quiet", "--initial-branch=main"]);
        git_ok(&local, &["config", "user.name", "Kanban Test"]);
        git_ok(
            &local,
            &["config", "user.email", "kanban-test@example.invalid"],
        );
        git_ok(&local, &["add", "--all"]);
        git_ok(&local, &["commit", "--quiet", "-m", "initial"]);

        git_ok(
            &bare,
            &["init", "--quiet", "--bare", "--initial-branch=main"],
        );
        let bare_url = bare.to_string_lossy().into_owned();
        git_ok(&local, &["remote", "add", "origin", &bare_url]);
        git_ok(
            &local,
            &["push", "--quiet", "--set-upstream", "origin", "main"],
        );

        let clone = Command::new("git")
            .args(["clone", "--quiet", "--branch", "main"])
            .arg(&bare)
            .arg(&upstream)
            .output()
            .expect("clone should run");
        assert!(
            clone.status.success(),
            "clone failed: {}",
            String::from_utf8_lossy(&clone.stderr)
        );
        git_ok(&upstream, &["config", "user.name", "Kanban Test"]);
        git_ok(
            &upstream,
            &["config", "user.email", "kanban-test@example.invalid"],
        );

        GitFixture {
            _local_dir: local_dir,
            _bare_dir: bare_dir,
            _upstream_dir: upstream_dir,
            local,
            bare,
            upstream,
        }
    }

    fn advance_upstream(fixture: &GitFixture) {
        fs::write(
            fixture.upstream.join(SPRINT_PATH),
            sprint_markdown("Generated roster v2", "Keep this note."),
        )
        .expect("update upstream sprint file");
        git_ok(&fixture.upstream, &["add", "--", SPRINT_PATH]);
        git_ok(
            &fixture.upstream,
            &["commit", "--quiet", "-m", "update roster"],
        );
        git_ok(&fixture.upstream, &["push", "--quiet"]);
    }

    fn local_sprint_paths() -> Vec<PathBuf> {
        vec![PathBuf::from(SPRINT_PATH)]
    }

    #[test]
    fn pull_recovers_only_generated_sprint_roster_changes() {
        let fixture = git_fixture();
        advance_upstream(&fixture);
        fs::write(
            fixture.local.join(SPRINT_PATH),
            sprint_markdown("Generated roster v2", "Keep this note."),
        )
        .expect("simulate locally regenerated roster");
        let ctx = GitContext::discover(&fixture.local).unwrap();

        let outcome = sync_pull(&ctx, &local_sprint_paths(), None).expect("safe pull should run");

        assert!(
            outcome.output.status.success(),
            "{}",
            output_text(&outcome.output)
        );
        assert_eq!(outcome.recovered_paths, vec![SPRINT_PATH]);
        assert!(!outcome.rebased_local_changes);
        assert_eq!(
            fs::read_to_string(fixture.local.join(SPRINT_PATH)).unwrap(),
            sprint_markdown("Generated roster v2", "Keep this note.")
        );
        assert!(
            git_ok(&fixture.local, &["status", "--porcelain"])
                .stdout
                .is_empty()
        );
    }

    #[test]
    fn pull_auto_commits_and_rebases_sprint_prose_and_backlog_changes() {
        let fixture = git_fixture();
        advance_upstream(&fixture);
        let edited_sprint = sprint_markdown("Generated roster v2", "User-authored note.");
        fs::write(fixture.local.join(SPRINT_PATH), &edited_sprint)
            .expect("edit sprint prose and roster");
        let untracked_path = fixture.local.join("delivery/backlog/local.md");
        fs::create_dir_all(untracked_path.parent().unwrap()).expect("create backlog directory");
        fs::write(&untracked_path, "untracked work\n").expect("write untracked work");
        let ctx = GitContext::discover(&fixture.local).unwrap();
        let store = PendingStore::new(fixture.local.clone(), ctx.pending_file());
        store
            .record(
                "local web changes".to_string(),
                vec![fixture.local.join(SPRINT_PATH), untracked_path.clone()],
            )
            .unwrap();

        let outcome =
            sync_pull(&ctx, &local_sprint_paths(), Some(&store)).expect("sync pull should succeed");

        assert!(
            outcome.output.status.success(),
            "{}",
            output_text(&outcome.output)
        );
        assert!(outcome.rebased_local_changes);
        assert_eq!(
            fs::read_to_string(fixture.local.join(SPRINT_PATH)).unwrap(),
            edited_sprint
        );
        assert_eq!(
            fs::read_to_string(untracked_path).unwrap(),
            "untracked work\n"
        );
        let state = upstream_state(&fixture.local);
        assert_eq!(state.behind, 0);
        assert_eq!(state.ahead, 1);
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn pull_recovers_staged_generated_roster_changes() {
        let fixture = git_fixture();
        advance_upstream(&fixture);
        let staged_roster = sprint_markdown("Generated roster v2", "Keep this note.");
        fs::write(fixture.local.join(SPRINT_PATH), &staged_roster)
            .expect("write staged roster change");
        git_ok(&fixture.local, &["add", "--", SPRINT_PATH]);
        let ctx = GitContext::discover(&fixture.local).unwrap();

        let outcome = sync_pull(&ctx, &local_sprint_paths(), None).expect("pull should succeed");

        assert!(
            outcome.output.status.success(),
            "{}",
            output_text(&outcome.output)
        );
        assert_eq!(outcome.recovered_paths, vec![SPRINT_PATH]);
        assert_eq!(
            fs::read_to_string(fixture.local.join(SPRINT_PATH)).unwrap(),
            staged_roster
        );
        assert!(
            git_ok(&fixture.local, &["status", "--porcelain"])
                .stdout
                .is_empty()
        );
    }

    #[test]
    fn pull_restores_generated_files_when_git_pull_fails() {
        let fixture = git_fixture();
        let original = sprint_markdown("Generated roster v2", "Keep this note.");
        fs::write(fixture.local.join(SPRINT_PATH), &original)
            .expect("simulate locally regenerated roster");
        let missing_remote = fixture.bare.join("missing").to_string_lossy().into_owned();
        git_ok(
            &fixture.local,
            &["remote", "set-url", "origin", &missing_remote],
        );
        let ctx = GitContext::discover(&fixture.local).unwrap();

        let outcome = sync_pull(&ctx, &local_sprint_paths(), None)
            .expect("Git failure should be returned as a pull result");

        assert!(!outcome.output.status.success());
        assert!(outcome.recovered_paths.is_empty());
        assert_eq!(
            fs::read_to_string(fixture.local.join(SPRINT_PATH)).unwrap(),
            original
        );
        assert!(
            String::from_utf8_lossy(&git_ok(&fixture.local, &["status", "--porcelain"]).stdout)
                .contains("delivery/sprints/S001.generated.md")
        );
    }
}
