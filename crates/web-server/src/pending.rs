use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use chrono::Local;
use kanban_core::atomic_write;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingChange {
    pub(crate) recorded_at: String,
    pub(crate) summary: String,
    pub(crate) paths: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PendingChanges {
    changes: Vec<PendingChange>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingStore {
    repo_root: PathBuf,
    file: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl PendingStore {
    pub(crate) fn new(repo_root: PathBuf, file: PathBuf) -> Self {
        Self {
            repo_root,
            file,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) fn record(&self, summary: String, paths: Vec<PathBuf>) -> Result<()> {
        let _guard = self.lock.lock().expect("pending change lock poisoned");
        let mut changes = self.load_unlocked()?;
        let paths = paths
            .iter()
            .map(|path| relative_path(&self.repo_root, path))
            .collect::<Result<Vec<_>>>()?;
        if paths.is_empty() {
            return Ok(());
        }
        changes.changes.push(PendingChange {
            recorded_at: Local::now().to_rfc3339(),
            summary: sanitize_summary(&summary),
            paths,
        });
        self.write_unlocked(&changes)
    }

    pub(crate) fn load(&self) -> Result<Vec<PendingChange>> {
        let _guard = self.lock.lock().expect("pending change lock poisoned");
        Ok(self.load_unlocked()?.changes)
    }

    pub(crate) fn clear(&self) -> Result<()> {
        let _guard = self.lock.lock().expect("pending change lock poisoned");
        self.write_unlocked(&PendingChanges::default())
    }

    fn load_unlocked(&self) -> Result<PendingChanges> {
        match std::fs::read_to_string(&self.file) {
            Ok(content) => serde_json::from_str(&content)
                .with_context(|| format!("parse pending web changes {}", self.file.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(PendingChanges::default())
            }
            Err(error) => Err(error)
                .with_context(|| format!("read pending web changes {}", self.file.display())),
        }
    }

    fn write_unlocked(&self, changes: &PendingChanges) -> Result<()> {
        let content = serde_json::to_string(changes).context("serialize pending web changes")?;
        atomic_write(&self.file, &content)
    }
}

pub(crate) fn commit_paths(changes: &[PendingChange]) -> Result<Vec<String>> {
    let mut paths = BTreeSet::new();
    for path in changes.iter().flat_map(|change| &change.paths) {
        let candidate = Path::new(path);
        if candidate.is_absolute()
            || path.starts_with('-')
            || candidate
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            bail!("invalid pending commit path");
        }
        paths.insert(path.clone());
    }
    Ok(paths.into_iter().collect())
}

pub(crate) fn commit_message(changes: &[PendingChange]) -> String {
    let count = changes.len();
    let noun = if count == 1 { "update" } else { "updates" };
    let mut message = format!("kanban: {count} backlog {noun} from web UI");
    for change in changes.iter().take(200) {
        message.push_str(&format!("\n\n- {} {}", change.recorded_at, change.summary));
    }
    if count > 200 {
        message.push_str(&format!("\n\n- and {} more", count - 200));
    }
    message
}

fn relative_path(repo_root: &Path, path: &Path) -> Result<String> {
    let path = if path.is_absolute() {
        path.strip_prefix(repo_root)
            .context("changed path is outside repository")?
    } else {
        path
    };
    let value = path.to_string_lossy().replace('\\', "/");
    if value.is_empty()
        || value.starts_with('-')
        || Path::new(&value)
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        bail!("invalid changed path");
    }
    Ok(value)
}

fn sanitize_summary(summary: &str) -> String {
    summary
        .replace(['\r', '\n'], " ")
        .chars()
        .take(160)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_paths_are_sorted_and_reject_escape_paths() {
        let changes = vec![PendingChange {
            recorded_at: "now".to_string(),
            summary: "test".to_string(),
            paths: vec!["b.md".to_string(), "a.md".to_string(), "a.md".to_string()],
        }];
        assert_eq!(commit_paths(&changes).unwrap(), vec!["a.md", "b.md"]);
        assert!(
            commit_paths(&[PendingChange {
                paths: vec!["../escape".to_string()],
                ..changes[0].clone()
            }])
            .is_err()
        );
    }

    #[test]
    fn message_is_human_readable() {
        let changes = vec![PendingChange {
            recorded_at: "now".to_string(),
            summary: "first".to_string(),
            paths: vec!["a.md".to_string()],
        }];
        assert_eq!(
            commit_message(&changes),
            "kanban: 1 backlog update from web UI\n\n- now first"
        );
    }
}
