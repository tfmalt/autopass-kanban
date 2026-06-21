use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

const CONFIG_DIR_NAME: &str = ".kanban";
const SETTINGS_FILE_NAME: &str = "settings.json";
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
    #[serde(default)]
    pub features: Option<FeaturesConfig>,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            backlog: DEFAULT_BACKLOG_PATH.to_string(),
            sprints: DEFAULT_SPRINTS_PATH.to_string(),
            features: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeaturesConfig {
    #[serde(default = "default_feature_on")]
    pub phases: bool,
    #[serde(default = "default_feature_on")]
    pub sprints: bool,
    #[serde(default = "default_feature_on")]
    pub epics: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            phases: true,
            sprints: true,
            epics: true,
        }
    }
}

fn default_feature_on() -> bool {
    true
}

impl FeaturesConfig {
    pub fn all_enabled() -> Self {
        Self::default()
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

    pub fn features(&self) -> FeaturesConfig {
        self.paths.features.unwrap_or_default()
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
    init_config_with_features(repo_root, None)
}

pub fn init_config_with_features(
    repo_root: impl AsRef<Path>,
    features: Option<FeaturesConfig>,
) -> Result<ConfigInitResult> {
    let repo_root = resolve_repo_root(repo_root)?;
    let config_dir = repo_root.join(CONFIG_DIR_NAME);
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("create config directory {}", config_dir.display()))?;

    let paths_default = PathsConfig {
        features,
        ..PathsConfig::default()
    };
    validate_paths(&paths_default)?;

    let mut created_files = Vec::new();
    created_files.extend(write_default_json_if_missing(
        &repo_root,
        &config_dir.join(PATHS_FILE_NAME),
        &paths_default,
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
        "features.sprints" => Ok(config.features().sprints.to_string()),
        "features.epics" => Ok(config.features().epics.to_string()),
        "features.phases" => Ok(config.features().phases.to_string()),
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
    let allow_empty = key == "paths.sprints";
    if trimmed_value.is_empty() && !allow_empty {
        bail!("Configuration values must not be empty.");
    }

    let settings_path = config_dir.join(SETTINGS_FILE_NAME);
    let contents = fs::read_to_string(&settings_path)
        .with_context(|| format!("read config file {}", settings_path.display()))?;
    let mut settings = serde_json::from_str::<Settings>(&contents)
        .with_context(|| format!("parse config file {}", settings_path.display()))?;

    match key {
        "paths.backlog" => {
            settings.paths.backlog = normalize_relative_repo_path(trimmed_value)?;
            validate_paths(&settings.paths)?;
        }
        "paths.sprints" => {
            settings.paths.sprints = if trimmed_value.is_empty() {
                String::new()
            } else {
                normalize_relative_repo_path(trimmed_value)?
            };
            validate_paths(&settings.paths)?;
        }
        "features.sprints" | "features.epics" | "features.phases" => {
            let enabled = parse_feature_flag(trimmed_value)?;
            let mut features = settings.paths.features.unwrap_or_default();
            match key {
                "features.sprints" => features.sprints = enabled,
                "features.epics" => features.epics = enabled,
                "features.phases" => features.phases = enabled,
                _ => unreachable!(),
            }
            settings.paths.features = Some(features);
            validate_paths(&settings.paths)?;
        }
        "theme.color_mode" => {
            settings.theme.color_mode = parse_color_mode(trimmed_value)?;
        }
        "web.port" => {
            let port: u16 = trimmed_value
                .parse()
                .map_err(|_| anyhow!("web.port must be a number between 1 and 65535."))?;
            if port == 0 {
                bail!("web.port must be a number between 1 and 65535.");
            }
            settings.web.port = port;
        }
        "web.host" => {
            settings.web.host = trimmed_value.to_string();
        }
        "web.style" => {
            if !WEB_STYLES.contains(&trimmed_value) {
                bail!("web.style must be one of: {}.", WEB_STYLES.join(", "));
            }
            settings.web.style = trimmed_value.to_string();
        }
        "story_points.allowed_values" => {
            settings.story_points.allowed_values = trimmed_value
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect();
            settings.story_points = settings.story_points.normalize_and_validate()?;
        }
        _ if key.starts_with("story_points.aliases.") => {
            let alias = key.trim_start_matches("story_points.aliases.").trim();
            if alias.is_empty() {
                bail!("Story point alias keys must not be empty.");
            }
            settings
                .story_points
                .aliases
                .insert(alias.to_string(), trimmed_value.to_string());
            settings.story_points = settings.story_points.normalize_and_validate()?;
        }
        _ => return unsupported_key(key),
    };

    write_json(&settings_path, &settings)?;

    Ok(ConfigSetResult {
        repo_root: repo_root.clone(),
        file_path: relative_path(&repo_root, &settings_path),
        key: key.to_string(),
        value: get_config_value(&repo_root, key)?,
    })
}

fn read_settings(repo_root: &Path) -> Result<Settings> {
    let config_dir = repo_root.join(CONFIG_DIR_NAME);
    let settings_file = config_dir.join(SETTINGS_FILE_NAME);
    let contents = fs::read_to_string(&settings_file)
        .with_context(|| format!("read config file {}", settings_file.display()))?;
    serde_json::from_str::<Settings>(&contents)
        .with_context(|| format!("parse config file {}", settings_file.display()))
}

fn load_kanban_config_from_root(repo_root: &Path) -> Result<KanbanConfig> {
    let config_dir = repo_root.join(CONFIG_DIR_NAME);
    if !config_dir.is_dir() {
        return missing_config_error(repo_root);
    }

    read_settings(repo_root)?.into_config(repo_root.to_path_buf())
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
    if backlog.is_empty() {
        bail!("Configured paths must not be empty.");
    }
    let sprints_enabled = paths.features.map(|f| f.sprints).unwrap_or(true);
    if sprints_enabled {
        if paths.sprints.trim().is_empty() {
            bail!(
                "Configured paths.sprints must not be empty when the sprints feature is enabled."
            );
        }
        let sprints = normalize_relative_repo_path(&paths.sprints)?;
        if sprints.is_empty() {
            bail!(
                "Configured paths.sprints must not be empty when the sprints feature is enabled."
            );
        }
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

fn parse_feature_flag(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "1" | "yes" => Ok(true),
        "false" | "off" | "0" | "no" => Ok(false),
        _ => bail!("Feature flag must be one of: true, false, on, off, yes, no, 1, 0."),
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
        "Unsupported config key `{key}`. Supported keys: paths.backlog, paths.sprints, features.sprints, features.epics, features.phases, theme.color_mode, story_points.allowed_values, story_points.aliases.<NAME>, web.port, web.host, web.style."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_config_creates_settings() {
        let temp_root = tempdir().unwrap();

        let result = init_config(temp_root.path()).unwrap();

        assert_eq!(result.config_dir, PathBuf::from(".kanban"));
        assert!(
            result
                .created_files
                .contains(&PathBuf::from(".kanban/settings.json"))
        );
        assert!(temp_root.path().join(".kanban/settings.json").exists());
    }

    #[test]
    fn set_config_value_updates_story_point_alias() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        let result = set_config_value(temp_root.path(), "story_points.aliases.XXL", "21").unwrap();
        let config = load_kanban_config(temp_root.path()).unwrap();

        assert_eq!(result.file_path, PathBuf::from(".kanban/settings.json"));
        assert_eq!(
            config.story_points.aliases.get("XXL").map(String::as_str),
            Some("21")
        );
        assert!(config.story_points.accepted_values().contains("XXL"));
        assert!(config.story_points.accepted_values().contains("21"));
    }

    #[test]
    fn init_config_creates_settings_with_defaults() {
        let temp_root = tempdir().unwrap();

        let result = init_config(temp_root.path()).unwrap();

        assert!(
            result
                .created_files
                .contains(&PathBuf::from(".kanban/settings.json"))
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
        assert_eq!(result.file_path, PathBuf::from(".kanban/settings.json"));

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

    #[test]
    fn features_default_to_all_enabled_when_block_missing() {
        let config = KanbanConfig {
            repo_root: PathBuf::from("/tmp/repo"),
            paths: PathsConfig::default(),
            theme: ThemeConfig::default(),
            story_points: StoryPointsConfig::default(),
            web: WebConfig::default(),
        };
        let features = config.features();
        assert!(features.phases);
        assert!(features.sprints);
        assert!(features.epics);
    }

    #[test]
    fn features_round_trip_through_set_config_value() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        for (key, value) in [
            ("features.sprints", "false"),
            ("features.epics", "off"),
            ("features.phases", "no"),
        ] {
            set_config_value(temp_root.path(), key, value).unwrap();
        }

        let config = load_kanban_config(temp_root.path()).unwrap();
        let features = config.features();
        assert!(!features.sprints);
        assert!(!features.epics);
        assert!(!features.phases);
        assert_eq!(
            get_config_value(temp_root.path(), "features.sprints").unwrap(),
            "false"
        );
        assert_eq!(
            get_config_value(temp_root.path(), "features.epics").unwrap(),
            "false"
        );
        assert_eq!(
            get_config_value(temp_root.path(), "features.phases").unwrap(),
            "false"
        );
    }

    #[test]
    fn set_feature_flag_rejects_unknown_value() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        let err = set_config_value(temp_root.path(), "features.sprints", "maybe").unwrap_err();
        assert!(err.to_string().contains("Feature flag"));
    }

    #[test]
    fn empty_sprints_path_allowed_when_sprints_feature_disabled() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        set_config_value(temp_root.path(), "features.sprints", "false").unwrap();
        set_config_value(temp_root.path(), "paths.sprints", "").unwrap();

        let config = load_kanban_config(temp_root.path()).unwrap();
        assert!(!config.features().sprints);
    }

    #[test]
    fn empty_sprints_path_rejected_when_sprints_feature_enabled() {
        let temp_root = tempdir().unwrap();
        init_config(temp_root.path()).unwrap();

        set_config_value(temp_root.path(), "features.sprints", "true").unwrap();
        let err = set_config_value(temp_root.path(), "paths.sprints", "").unwrap_err();
        assert!(err.to_string().contains("paths.sprints"));
    }
}
