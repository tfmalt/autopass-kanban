use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

const CONFIG_DIR_NAME: &str = ".kanban";
const PATHS_FILE_NAME: &str = "paths.json";
const THEME_FILE_NAME: &str = "theme.json";
const STORY_POINTS_FILE_NAME: &str = "story-points.json";
const WEB_FILE_NAME: &str = "web.json";
const DEFAULT_BACKLOG_PATH: &str = "delivery/backlog";
const DEFAULT_SPRINTS_PATH: &str = "delivery/sprints";
const DEFAULT_WEB_PORT: u16 = 3000;
const DEFAULT_WEB_HOST: &str = "127.0.0.1";
const DEFAULT_WEB_STYLE: &str = "calm-light";
const WEB_STYLES: [&str; 3] = ["calm-light", "modern-dark", "vibrant"];
const DEFAULT_ALLOWED_STORY_POINTS: [&str; 5] = ["2", "3", "5", "8", "13"];
const DEFAULT_STORY_POINT_ALIASES: [(&str, &str); 5] = [
    ("XS", "2"),
    ("S", "3"),
    ("M", "5"),
    ("L", "8"),
    ("XL", "13"),
];

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathsConfig {
    pub backlog: String,
    pub sprints: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            backlog: DEFAULT_BACKLOG_PATH.to_string(),
            sprints: DEFAULT_SPRINTS_PATH.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub color_mode: ColorMode,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            color_mode: ColorMode::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryPointsConfig {
    pub allowed_values: Vec<String>,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

impl Default for StoryPointsConfig {
    fn default() -> Self {
        Self {
            allowed_values: DEFAULT_ALLOWED_STORY_POINTS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            aliases: DEFAULT_STORY_POINT_ALIASES
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        }
    }
}

impl StoryPointsConfig {
    pub fn accepted_values(&self) -> BTreeSet<String> {
        self.allowed_values
            .iter()
            .cloned()
            .chain(self.aliases.keys().cloned())
            .chain(self.aliases.values().cloned())
            .collect()
    }

    pub fn display_value(&self, value: &str) -> String {
        match self.aliases.get(value.trim()) {
            Some(mapped) if mapped != value.trim() => format!("{} ({mapped})", value.trim()),
            _ => value.trim().to_string(),
        }
    }

    fn normalize_and_validate(mut self) -> Result<Self> {
        self.allowed_values = self
            .allowed_values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        self.allowed_values.sort();
        self.allowed_values.dedup();
        if self.allowed_values.is_empty() {
            bail!("story_points.allowed_values must contain at least one value.");
        }

        let mut normalized_aliases = BTreeMap::new();
        for (raw_key, raw_value) in self.aliases {
            let key = raw_key.trim().to_string();
            let value = raw_value.trim().to_string();
            if key.is_empty() {
                bail!("story_points.aliases keys must not be empty.");
            }
            if value.is_empty() {
                bail!("story_points.aliases values must not be empty.");
            }
            normalized_aliases.insert(key, value);
        }
        self.aliases = normalized_aliases;
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebConfig {
    pub port: u16,
    pub host: String,
    pub style: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_WEB_PORT,
            host: DEFAULT_WEB_HOST.to_string(),
            style: DEFAULT_WEB_STYLE.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KanbanConfig {
    #[serde(skip_serializing)]
    pub repo_root: PathBuf,
    pub paths: PathsConfig,
    pub theme: ThemeConfig,
    pub story_points: StoryPointsConfig,
    pub web: WebConfig,
}

impl KanbanConfig {
    pub fn backlog_path(&self) -> PathBuf {
        self.repo_root.join(&self.paths.backlog)
    }

    pub fn sprints_path(&self) -> PathBuf {
        self.repo_root.join(&self.paths.sprints)
    }

    pub fn backlog_marker(&self) -> String {
        path_marker(&self.paths.backlog)
    }

    pub fn sprints_marker(&self) -> String {
        path_marker(&self.paths.sprints)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigInitResult {
    pub repo_root: PathBuf,
    pub config_dir: PathBuf,
    pub created_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSetResult {
    pub repo_root: PathBuf,
    pub file_path: PathBuf,
    pub key: String,
    pub value: String,
}

pub fn resolve_repo_root(path: impl AsRef<Path>) -> Result<PathBuf> {
    let candidate = fs::canonicalize(path.as_ref())
        .with_context(|| format!("resolve repository path {}", path.as_ref().display()))?;
    if let Some(root) = git_toplevel(&candidate) {
        return Ok(root);
    }
    Ok(candidate)
}

pub fn init_config(repo_root: impl AsRef<Path>) -> Result<ConfigInitResult> {
    let repo_root = resolve_repo_root(repo_root)?;
    let config_dir = repo_root.join(CONFIG_DIR_NAME);
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("create config directory {}", config_dir.display()))?;

    let mut created_files = Vec::new();
    created_files.extend(write_default_json_if_missing(
        &repo_root,
        &config_dir.join(PATHS_FILE_NAME),
        &PathsConfig::default(),
    )?);
    created_files.extend(write_default_json_if_missing(
        &repo_root,
        &config_dir.join(THEME_FILE_NAME),
        &ThemeConfig::default(),
    )?);
    created_files.extend(write_default_json_if_missing(
        &repo_root,
        &config_dir.join(STORY_POINTS_FILE_NAME),
        &StoryPointsConfig::default(),
    )?);
    created_files.extend(write_default_json_if_missing(
        &repo_root,
        &config_dir.join(WEB_FILE_NAME),
        &WebConfig::default(),
    )?);

    Ok(ConfigInitResult {
        repo_root: repo_root.clone(),
        config_dir: relative_path(&repo_root, &config_dir),
        created_files,
    })
}

pub fn load_kanban_config(repo_root: impl AsRef<Path>) -> Result<KanbanConfig> {
    let repo_root = resolve_repo_root(repo_root)?;
    load_kanban_config_from_root(&repo_root)
}

pub fn get_config_json(repo_root: impl AsRef<Path>) -> Result<String> {
    let config = load_kanban_config(repo_root)?;
    serde_json::to_string_pretty(&config).context("serialize kanban config")
}

pub fn get_config_value(repo_root: impl AsRef<Path>, key: &str) -> Result<String> {
    let config = load_kanban_config(repo_root)?;
    let key = key.trim();
    match key {
        "paths.backlog" => Ok(config.paths.backlog),
        "paths.sprints" => Ok(config.paths.sprints),
        "theme.color_mode" => Ok(match config.theme.color_mode {
            ColorMode::Auto => "auto".to_string(),
            ColorMode::Always => "always".to_string(),
            ColorMode::Never => "never".to_string(),
        }),
        "web.port" => Ok(config.web.port.to_string()),
        "web.host" => Ok(config.web.host),
        "web.style" => Ok(config.web.style),
        "story_points.allowed_values" => {
            serde_json::to_string_pretty(&config.story_points.allowed_values)
                .context("serialize story point values")
        }
        _ if key.starts_with("story_points.aliases.") => {
            let alias = key.trim_start_matches("story_points.aliases.");
            config
                .story_points
                .aliases
                .get(alias)
                .cloned()
                .ok_or_else(|| anyhow!("Unknown story point alias: {alias}"))
        }
        _ => unsupported_key(key),
    }
}

pub fn set_config_value(
    repo_root: impl AsRef<Path>,
    key: &str,
    value: &str,
) -> Result<ConfigSetResult> {
    let repo_root = resolve_repo_root(repo_root)?;
    let config_dir = repo_root.join(CONFIG_DIR_NAME);
    if !config_dir.is_dir() {
        bail!(
            "No `.kanban` configuration found in {}. Run `kanban init`.",
            repo_root.display()
        );
    }

    let key = key.trim();
    let trimmed_value = value.trim();
    if trimmed_value.is_empty() {
        bail!("Configuration values must not be empty.");
    }

    let mut paths = read_json_or_default::<PathsConfig>(&config_dir.join(PATHS_FILE_NAME))?;
    let mut theme = read_json_or_default::<ThemeConfig>(&config_dir.join(THEME_FILE_NAME))?;
    let mut story_points =
        read_json_or_default::<StoryPointsConfig>(&config_dir.join(STORY_POINTS_FILE_NAME))?;
    let mut web = read_json_or_default::<WebConfig>(&config_dir.join(WEB_FILE_NAME))?;

    let file_path = match key {
        "paths.backlog" => {
            paths.backlog = normalize_relative_repo_path(trimmed_value)?;
            validate_paths(&paths)?;
            write_json(&config_dir.join(PATHS_FILE_NAME), &paths)?;
            config_dir.join(PATHS_FILE_NAME)
        }
        "paths.sprints" => {
            paths.sprints = normalize_relative_repo_path(trimmed_value)?;
            validate_paths(&paths)?;
            write_json(&config_dir.join(PATHS_FILE_NAME), &paths)?;
            config_dir.join(PATHS_FILE_NAME)
        }
        "theme.color_mode" => {
            theme.color_mode = parse_color_mode(trimmed_value)?;
            write_json(&config_dir.join(THEME_FILE_NAME), &theme)?;
            config_dir.join(THEME_FILE_NAME)
        }
        "web.port" => {
            let port: u16 = trimmed_value
                .parse()
                .map_err(|_| anyhow!("web.port must be a number between 1 and 65535."))?;
            if port == 0 {
                bail!("web.port must be a number between 1 and 65535.");
            }
            web.port = port;
            write_json(&config_dir.join(WEB_FILE_NAME), &web)?;
            config_dir.join(WEB_FILE_NAME)
        }
        "web.host" => {
            web.host = trimmed_value.to_string();
            write_json(&config_dir.join(WEB_FILE_NAME), &web)?;
            config_dir.join(WEB_FILE_NAME)
        }
        "web.style" => {
            if !WEB_STYLES.contains(&trimmed_value) {
                bail!("web.style must be one of: {}.", WEB_STYLES.join(", "));
            }
            web.style = trimmed_value.to_string();
            write_json(&config_dir.join(WEB_FILE_NAME), &web)?;
            config_dir.join(WEB_FILE_NAME)
        }
        "story_points.allowed_values" => {
            story_points.allowed_values = trimmed_value
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect();
            story_points = story_points.normalize_and_validate()?;
            write_json(&config_dir.join(STORY_POINTS_FILE_NAME), &story_points)?;
            config_dir.join(STORY_POINTS_FILE_NAME)
        }
        _ if key.starts_with("story_points.aliases.") => {
            let alias = key.trim_start_matches("story_points.aliases.").trim();
            if alias.is_empty() {
                bail!("Story point alias keys must not be empty.");
            }
            story_points
                .aliases
                .insert(alias.to_string(), trimmed_value.to_string());
            story_points = story_points.normalize_and_validate()?;
            write_json(&config_dir.join(STORY_POINTS_FILE_NAME), &story_points)?;
            config_dir.join(STORY_POINTS_FILE_NAME)
        }
        _ => return unsupported_key(key),
    };

    Ok(ConfigSetResult {
        repo_root: repo_root.clone(),
        file_path: relative_path(&repo_root, &file_path),
        key: key.to_string(),
        value: get_config_value(&repo_root, key)?,
    })
}

fn load_kanban_config_from_root(repo_root: &Path) -> Result<KanbanConfig> {
    let config_dir = repo_root.join(CONFIG_DIR_NAME);
    if !config_dir.is_dir() {
        return missing_config_error(repo_root);
    }

    let paths = read_json_or_default::<PathsConfig>(&config_dir.join(PATHS_FILE_NAME))?;
    validate_paths(&paths)?;
    let story_points =
        read_json_or_default::<StoryPointsConfig>(&config_dir.join(STORY_POINTS_FILE_NAME))?
            .normalize_and_validate()?;
    let theme = read_json_or_default::<ThemeConfig>(&config_dir.join(THEME_FILE_NAME))?;
    let web = read_json_or_default::<WebConfig>(&config_dir.join(WEB_FILE_NAME))?;

    Ok(KanbanConfig {
        repo_root: repo_root.to_path_buf(),
        paths,
        theme,
        story_points,
        web,
    })
}

#[cfg(test)]
fn missing_config_error(repo_root: &Path) -> Result<KanbanConfig> {
    Ok(KanbanConfig {
        repo_root: repo_root.to_path_buf(),
        paths: PathsConfig::default(),
        theme: ThemeConfig::default(),
        story_points: StoryPointsConfig::default(),
        web: WebConfig::default(),
    })
}

#[cfg(not(test))]
fn missing_config_error(repo_root: &Path) -> Result<KanbanConfig> {
    bail!(
        "No `.kanban` configuration found in {}. Run `kanban init` to initialize this repository.",
        repo_root.display()
    )
}

fn git_toplevel(path: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }
    fs::canonicalize(root).ok()
}

fn read_json_or_default<T>(file_path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !file_path.exists() {
        return Ok(T::default());
    }
    let contents = fs::read_to_string(file_path)
        .with_context(|| format!("read config file {}", file_path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("parse config file {}", file_path.display()))
}

fn write_default_json_if_missing<T>(
    repo_root: &Path,
    file_path: &Path,
    value: &T,
) -> Result<Vec<PathBuf>>
where
    T: Serialize,
{
    if file_path.exists() {
        return Ok(Vec::new());
    }
    write_json(file_path, value)?;
    Ok(vec![relative_path(repo_root, file_path)])
}

fn write_json<T>(file_path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    let json = serde_json::to_string_pretty(value).context("serialize config json")?;
    fs::write(file_path, format!("{json}\n"))
        .with_context(|| format!("write config file {}", file_path.display()))
}

fn validate_paths(paths: &PathsConfig) -> Result<()> {
    let backlog = normalize_relative_repo_path(&paths.backlog)?;
    let sprints = normalize_relative_repo_path(&paths.sprints)?;
    if backlog.is_empty() || sprints.is_empty() {
        bail!("Configured paths must not be empty.");
    }
    Ok(())
}

fn normalize_relative_repo_path(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("Configured repository paths must not be empty.");
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        bail!("Configured repository paths must be relative to the repository root.");
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            bail!("Configured repository paths must stay inside the repository root.");
        }
    }

    Ok(trimmed.trim_matches('/').to_string())
}

fn parse_color_mode(value: &str) -> Result<ColorMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(ColorMode::Auto),
        "always" => Ok(ColorMode::Always),
        "never" => Ok(ColorMode::Never),
        _ => bail!("theme.color_mode must be one of: auto, always, never."),
    }
}

fn path_marker(relative_path: &str) -> String {
    format!("/{}/", relative_path.replace('\\', "/").trim_matches('/'))
}

fn relative_path(repo_root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(repo_root).unwrap_or(path).to_path_buf()
}

fn unsupported_key<T>(key: &str) -> Result<T> {
    bail!(
        "Unsupported config key `{key}`. Supported keys: paths.backlog, paths.sprints, theme.color_mode, story_points.allowed_values, story_points.aliases.<NAME>, web.port, web.host, web.style."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_config_creates_default_files() {
        let temp_root = tempdir().unwrap();

        let result = init_config(temp_root.path()).unwrap();

        assert_eq!(result.config_dir, PathBuf::from(".kanban"));
        assert!(
            result
                .created_files
                .contains(&PathBuf::from(".kanban/paths.json"))
        );
        assert!(temp_root.path().join(".kanban/story-points.json").exists());
    }

    #[test]
    fn set_config_value_updates_story_point_alias() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        let result = set_config_value(temp_root.path(), "story_points.aliases.XXL", "21").unwrap();
        let config = load_kanban_config(temp_root.path()).unwrap();

        assert_eq!(result.file_path, PathBuf::from(".kanban/story-points.json"));
        assert_eq!(
            config.story_points.aliases.get("XXL").map(String::as_str),
            Some("21")
        );
        assert!(config.story_points.accepted_values().contains("XXL"));
        assert!(config.story_points.accepted_values().contains("21"));
    }

    #[test]
    fn init_config_creates_web_json_with_defaults() {
        let temp_root = tempdir().unwrap();

        let result = init_config(temp_root.path()).unwrap();

        assert!(
            result
                .created_files
                .contains(&PathBuf::from(".kanban/web.json"))
        );
        let config = load_kanban_config(temp_root.path()).unwrap();
        assert_eq!(config.web.port, 3000);
        assert_eq!(config.web.host, "127.0.0.1");
        assert_eq!(config.web.style, "calm-light");
    }

    #[test]
    fn set_and_get_web_port_round_trips() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        let result = set_config_value(temp_root.path(), "web.port", "4000").unwrap();
        assert_eq!(result.file_path, PathBuf::from(".kanban/web.json"));

        let config = load_kanban_config(temp_root.path()).unwrap();
        assert_eq!(config.web.port, 4000);
        assert_eq!(
            get_config_value(temp_root.path(), "web.port").unwrap(),
            "4000"
        );
    }

    #[test]
    fn set_web_port_rejects_non_numeric() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        let err = set_config_value(temp_root.path(), "web.port", "abc").unwrap_err();
        assert!(err.to_string().contains("web.port"));
    }

    #[test]
    fn set_web_style_rejects_unknown_value() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        let err = set_config_value(temp_root.path(), "web.style", "neon").unwrap_err();
        assert!(err.to_string().contains("web.style"));
    }
}
