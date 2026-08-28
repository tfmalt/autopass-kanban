use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::{load_kanban_config, require_git_repo_root};
use crate::repository::atomic_write;

pub const KANBAN_CONFIG_HOME: &str = "KANBAN_CONFIG_HOME";
pub const KANBAN_REPO_ROOT: &str = "KANBAN_REPO_ROOT";
pub const DEFAULT_REPO_ROOT_MARKER: &str = "__kanban_default_root__";
const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub default_repo_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlobalConfigStatus {
    pub file_path: PathBuf,
    pub default_repo_root: Option<PathBuf>,
}

pub fn user_config_path() -> Result<PathBuf> {
    let config_home = env::var_os(KANBAN_CONFIG_HOME)
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .context("cannot determine kanban user configuration directory; set KANBAN_CONFIG_HOME")?;
    Ok(config_home.join("kanban").join(CONFIG_FILE_NAME))
}

pub fn global_config_status() -> Result<GlobalConfigStatus> {
    let file_path = user_config_path()?;
    let config = load_user_config_at(&file_path)?;
    Ok(GlobalConfigStatus {
        file_path,
        default_repo_root: config.default_repo_root,
    })
}

pub fn set_default_repo_root(path: impl AsRef<Path>) -> Result<GlobalConfigStatus> {
    let root = require_git_repo_root(path)?;
    load_kanban_config(&root).with_context(|| {
        format!(
            "configured default repository {} must contain .kanban/settings.json",
            root.display()
        )
    })?;

    let file_path = user_config_path()?;
    write_user_config(
        &file_path,
        &UserConfig {
            default_repo_root: Some(root.clone()),
        },
    )?;
    Ok(GlobalConfigStatus {
        file_path,
        default_repo_root: Some(root),
    })
}

pub fn clear_default_repo_root() -> Result<GlobalConfigStatus> {
    let file_path = user_config_path()?;
    if file_path.exists() {
        write_user_config(&file_path, &UserConfig::default())?;
    }
    Ok(GlobalConfigStatus {
        file_path,
        default_repo_root: None,
    })
}

/// Select a repository for an omitted CLI root argument. Explicit paths continue
/// to be resolved by the repository-local configuration loader.
pub fn effective_repo_root() -> Result<PathBuf> {
    if let Some(root) = env::var_os(KANBAN_REPO_ROOT) {
        return configured_repo_root(PathBuf::from(root), "KANBAN_REPO_ROOT");
    }

    if let Some(root) = local_repo_root()? {
        return Ok(root);
    }

    let status = global_config_status()?;
    if let Some(root) = status.default_repo_root {
        return configured_repo_root(root, &status.file_path.display().to_string());
    }

    Ok(PathBuf::from("."))
}

fn local_repo_root() -> Result<Option<PathBuf>> {
    let cwd = env::current_dir().context("read current directory")?;
    let Ok(root) = require_git_repo_root(&cwd) else {
        return Ok(None);
    };
    Ok(root.join(".kanban/settings.json").is_file().then_some(root))
}

fn configured_repo_root(path: PathBuf, source: &str) -> Result<PathBuf> {
    if !path.is_dir() {
        bail!(
            "Configured kanban repository {} from {source} is not a directory. Update it with `kanban config global set-root <PATH>`.",
            path.display()
        );
    }
    let root = require_git_repo_root(&path).with_context(|| {
        format!(
            "configured kanban repository {} from {source}",
            path.display()
        )
    })?;
    if !root.join(".kanban/settings.json").is_file() {
        bail!(
            "Configured kanban repository {} from {source} has no .kanban/settings.json. Update it with `kanban config global set-root <PATH>`.",
            root.display()
        );
    }
    Ok(root)
}

fn load_user_config_at(file_path: &Path) -> Result<UserConfig> {
    if !file_path.exists() {
        return Ok(UserConfig::default());
    }
    let contents = fs::read_to_string(file_path)
        .with_context(|| format!("read user config file {}", file_path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("parse user config file {}", file_path.display()))
}

fn write_user_config(file_path: &Path, config: &UserConfig) -> Result<()> {
    let payload = format!(
        "{}\n",
        serde_json::to_string_pretty(config).context("serialize user config")?
    );
    atomic_write(file_path, &payload)
        .with_context(|| format!("write user config file {}", file_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn absent_user_config_is_empty() {
        let directory = tempdir().unwrap();
        assert_eq!(
            load_user_config_at(&directory.path().join(CONFIG_FILE_NAME)).unwrap(),
            UserConfig::default()
        );
    }

    #[test]
    fn malformed_user_config_names_its_path() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(CONFIG_FILE_NAME);
        fs::write(&path, "not json").unwrap();

        assert!(
            load_user_config_at(&path)
                .unwrap_err()
                .to_string()
                .contains(&path.display().to_string())
        );
    }
}
