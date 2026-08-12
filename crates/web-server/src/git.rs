use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
