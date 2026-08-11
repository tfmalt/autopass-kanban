#[cfg(test)]
use crate::web_process::process_is_kanban_web;
use crate::web_process::{
    WebProcessState, finish_stopped_web_process, force_kill_process, read_web_port_file,
    read_web_process_state, remove_pid_file, terminate_process, wait_for_process_exit,
    write_web_port_file,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::Duration;

use crate::json_out::forward_slashed_path;
use crate::layout::{DEFAULT_OUTPUT_WIDTH, detected_terminal_width};
use crate::render::inline::{InlineToken, push_wrapped_inline_message};
use crate::render::sprint::push_line;
use crate::render::table::{display_width, wrap_text};
use crate::theme::Theme;
use kanban_core::{ColorMode, load_kanban_config};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WebStatusDto {
    pub(crate) state: String,
    pub(crate) pid: Option<u32>,
    pub(crate) stale_pid: Option<u32>,
    pub(crate) url: String,
    pub(crate) pid_file: String,
    pub(crate) log_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WebStartDto {
    pub(crate) state: String,
    pub(crate) pid: u32,
    pub(crate) url: String,
    pub(crate) requested_port: u16,
    pub(crate) actual_port: u16,
    pub(crate) port_changed: bool,
    pub(crate) log_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WebStopDto {
    pub(crate) stopped: bool,
    pub(crate) before: WebStatusDto,
    pub(crate) after: WebStatusDto,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WebRestartDto {
    pub(crate) stopped_existing: bool,
    pub(crate) started: WebStartDto,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WebLogDto {
    pub(crate) exists: bool,
    pub(crate) path: String,
    pub(crate) line_count: usize,
    pub(crate) lines: Vec<String>,
    pub(crate) content: String,
}

pub(crate) fn web_status_json(repo_root: &Path) -> Result<WebStatusDto> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    let process_state = read_web_process_state(&paths)?;
    let status_port = match process_state {
        WebProcessState::Running(_) => read_web_port_file(&paths).unwrap_or(config.web.port),
        WebProcessState::Stopped | WebProcessState::Stale(_) => config.web.port,
    };
    let url = format!("http://{}:{}", config.web.host, status_port);
    let (state, pid, stale_pid) = match process_state {
        WebProcessState::Stopped => ("stopped".to_string(), None, None),
        WebProcessState::Running(pid) => ("running".to_string(), Some(pid), None),
        WebProcessState::Stale(pid) => ("stale".to_string(), None, pid),
    };
    Ok(WebStatusDto {
        state,
        pid,
        stale_pid,
        url,
        pid_file: forward_slashed_path(&paths.pid_file),
        log_file: forward_slashed_path(&paths.log_file),
    })
}

pub(crate) fn web_start_json(repo_root: &Path, open: bool, dev: bool) -> Result<WebStartDto> {
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root;
    let paths = web_runtime_paths(&repo_root);
    fs::create_dir_all(&paths.run_dir)
        .with_context(|| format!("create web runtime directory {}", paths.run_dir.display()))?;

    match read_web_process_state(&paths)? {
        WebProcessState::Running(pid) => bail!("kanban web is already running with PID {pid}."),
        WebProcessState::Stale(_) => remove_pid_file(&paths)?,
        WebProcessState::Stopped => {}
    }

    let port = resolve_web_port(&config.web.host, config.web.port)?;
    let url = format!("http://{}:{}", config.web.host, port.actual);
    let spec = build_web_start_command_spec(&repo_root, dev, &config.web.host, port.actual)?;
    write_web_port_file(&paths, port.actual)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
        .with_context(|| format!("open web log {}", paths.log_file.display()))?;
    let stderr = log
        .try_clone()
        .with_context(|| format!("clone web log handle {}", paths.log_file.display()))?;
    let mut command = process_from_spec(&spec);
    command
        .env("KANBAN_WEB_PORT", port.actual.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    command.process_group(0);
    let child = command
        .spawn()
        .with_context(|| format!("start web server with {}", spec.program))?;
    fs::write(&paths.pid_file, format!("{}\n", child.id()))
        .with_context(|| format!("write PID file {}", paths.pid_file.display()))?;

    if open {
        open_browser_url(&url)?;
    }

    Ok(WebStartDto {
        state: "running".to_string(),
        pid: child.id(),
        url,
        requested_port: port.requested,
        actual_port: port.actual,
        port_changed: port.changed(),
        log_file: forward_slashed_path(&paths.log_file),
    })
}

pub(crate) fn web_stop_json(repo_root: &Path) -> Result<WebStopDto> {
    let before = web_status_json(repo_root)?;
    let stopped = stop_web(&Theme::for_stdout(ColorMode::Never), repo_root, true)?;
    let after = web_status_json(repo_root)?;
    Ok(WebStopDto {
        stopped,
        before,
        after,
    })
}

pub(crate) fn web_log_json(repo_root: &Path, lines: Option<usize>) -> Result<WebLogDto> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    if !paths.log_file.exists() {
        return Ok(WebLogDto {
            exists: false,
            path: forward_slashed_path(&paths.log_file),
            line_count: 0,
            lines: Vec::new(),
            content: String::new(),
        });
    }

    let content = fs::read_to_string(&paths.log_file)
        .with_context(|| format!("read web log {}", paths.log_file.display()))?;
    let selected_lines = match lines {
        Some(0) => Vec::new(),
        Some(limit) => content
            .lines()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(str::to_string)
            .collect(),
        None => content.lines().map(str::to_string).collect(),
    };
    let selected_content = selected_lines.join("\n");
    let line_count = selected_lines.len();
    Ok(WebLogDto {
        exists: true,
        path: forward_slashed_path(&paths.log_file),
        line_count,
        lines: selected_lines,
        content: selected_content,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebRuntimePaths {
    pub(crate) run_dir: PathBuf,
    pub(crate) pid_file: PathBuf,
    pub(crate) port_file: PathBuf,
    pub(crate) log_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebStartCommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebPortResolution {
    pub(crate) requested: u16,
    pub(crate) actual: u16,
}

impl WebPortResolution {
    pub(crate) fn changed(&self) -> bool {
        self.requested != self.actual
    }
}

pub(crate) fn web_runtime_paths(repo_root: &Path) -> WebRuntimePaths {
    let run_dir = repo_root.join(".kanban/run");
    WebRuntimePaths {
        pid_file: run_dir.join("web.pid"),
        port_file: run_dir.join("web.port"),
        log_file: run_dir.join("web.log"),
        run_dir,
    }
}

fn is_kanban_tool_root(path: &Path) -> bool {
    path.join("Cargo.toml").is_file()
        && path.join("crates/cli/Cargo.toml").is_file()
        && path.join("web/package.json").is_file()
}

pub(crate) fn kanban_tool_root(repo_root: &Path) -> Result<PathBuf> {
    if let Some(configured) = std::env::var_os("KANBAN_SOURCE_ROOT") {
        let configured = PathBuf::from(configured);
        if is_kanban_tool_root(&configured) {
            return Ok(configured);
        }
    }

    let mut candidates = vec![repo_root.to_path_buf()];
    if let Some(parent) = repo_root.parent() {
        candidates.push(parent.join("autopass-kanban"));
    }

    for candidate in candidates {
        if is_kanban_tool_root(&candidate) {
            return Ok(candidate);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        for ancestor in current_exe.ancestors() {
            if is_kanban_tool_root(ancestor) {
                return Ok(ancestor.to_path_buf());
            }
        }
    }

    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        let manifest_dir = Path::new(manifest_dir);
        if let Some(tool_root) = manifest_dir.ancestors().nth(2)
            && is_kanban_tool_root(tool_root)
        {
            return Ok(tool_root.to_path_buf());
        }
    }

    bail!(
        "kanban source checkout not found. `kanban web start --dev` and `--build` require a kanban source tree with `web/package.json`."
    )
}

pub(crate) fn web_app_dir(repo_root: &Path) -> Result<PathBuf> {
    Ok(kanban_tool_root(repo_root)?.join("web"))
}

pub(crate) fn build_web_start_command_spec(
    repo_root: &Path,
    dev: bool,
    host: &str,
    port: u16,
) -> Result<WebStartCommandSpec> {
    let cwd = child_process_path(repo_root);
    if dev {
        let web_dir = child_process_path(&web_app_dir(repo_root)?);
        Ok(WebStartCommandSpec {
            program: npm_program(),
            args: vec![
                "--prefix".to_string(),
                web_dir.to_string_lossy().into_owned(),
                "run".to_string(),
                "dev".to_string(),
                "--".to_string(),
                "--host".to_string(),
                host.to_string(),
                "--port".to_string(),
                port.to_string(),
            ],
            cwd,
        })
    } else {
        Ok(WebStartCommandSpec {
            program: std::env::current_exe()
                .context("resolve current kanban executable")?
                .to_string_lossy()
                .into_owned(),
            args: vec![
                "web".to_string(),
                "serve".to_string(),
                "--repo-root".to_string(),
                cwd.to_string_lossy().into_owned(),
                "--host".to_string(),
                host.to_string(),
                "--port".to_string(),
                port.to_string(),
            ],
            cwd,
        })
    }
}

pub(crate) fn build_web_build_command_spec(repo_root: &Path) -> Result<WebStartCommandSpec> {
    let web_dir = child_process_path(&web_app_dir(repo_root)?);
    Ok(WebStartCommandSpec {
        program: npm_program(),
        args: vec![
            "--prefix".to_string(),
            web_dir.to_string_lossy().into_owned(),
            "run".to_string(),
            "build".to_string(),
        ],
        cwd: child_process_path(repo_root),
    })
}

#[cfg(windows)]
fn child_process_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix("\\\\?\\") {
        if let Some(unc) = rest.strip_prefix("UNC\\") {
            return PathBuf::from(format!("\\\\{unc}"));
        }
        return PathBuf::from(rest);
    }
    path.to_path_buf()
}

#[cfg(not(windows))]
fn child_process_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
fn npm_program() -> String {
    "npm.cmd".to_string()
}

#[cfg(not(windows))]
fn npm_program() -> String {
    "npm".to_string()
}

pub(crate) fn process_from_spec(spec: &WebStartCommandSpec) -> ProcessCommand {
    let mut command = ProcessCommand::new(&spec.program);
    command.args(&spec.args).current_dir(&spec.cwd);
    command
}

pub(crate) fn resolve_web_port(host: &str, requested: u16) -> Result<WebPortResolution> {
    for port in requested..=u16::MAX {
        match TcpListener::bind((host, port)) {
            Ok(listener) => {
                drop(listener);
                return Ok(WebPortResolution {
                    requested,
                    actual: port,
                });
            }
            Err(error) if error.kind() == ErrorKind::AddrInUse => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("check whether {host}:{port} is available"));
            }
        }
    }

    bail!("No available port found at or above {requested} on {host}.")
}

pub(crate) fn run_web_build(repo_root: &Path) -> Result<()> {
    let spec = build_web_build_command_spec(repo_root)?;
    let status = process_from_spec(&spec)
        .status()
        .with_context(|| format!("run {} {}", spec.program, spec.args.join(" ")))?;
    if !status.success() {
        bail!("web build failed with status {status}");
    }
    Ok(())
}

pub(crate) fn open_browser_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = ProcessCommand::new("open");
    #[cfg(target_os = "linux")]
    let mut command = ProcessCommand::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = ProcessCommand::new("cmd");
        command.arg("/C").arg("start");
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        bail!("opening a browser is not supported on this platform");
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        command.arg(url);
        let status = command
            .status()
            .with_context(|| format!("open browser URL {url}"))?;
        if !status.success() {
            bail!("open browser command failed with status {status}");
        }
        Ok(())
    }
}

pub(crate) fn start_web(
    theme: &Theme,
    repo_root: &Path,
    foreground: bool,
    open: bool,
    dev: bool,
    build: bool,
) -> Result<()> {
    if dev && build {
        bail!("--build cannot be combined with --dev.");
    }
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root;
    let paths = web_runtime_paths(&repo_root);
    fs::create_dir_all(&paths.run_dir)
        .with_context(|| format!("create web runtime directory {}", paths.run_dir.display()))?;

    match read_web_process_state(&paths)? {
        WebProcessState::Running(pid) => {
            eprint!(
                "{}",
                render_web_already_running_error(
                    theme,
                    pid,
                    detected_terminal_width().unwrap_or(DEFAULT_OUTPUT_WIDTH)
                )
            );
            std::process::exit(1);
        }
        WebProcessState::Stale(_) => remove_pid_file(&paths)?,
        WebProcessState::Stopped => {}
    }

    if build {
        println!("{} building kanban web UI...", theme.info_label());
        run_web_build(&repo_root)?;
    }
    let port = resolve_web_port(&config.web.host, config.web.port)?;
    if port.changed() {
        println!(
            "{}",
            render_web_port_fallback_warning(theme, &config.web.host, port.requested, port.actual)
        );
    }

    let url = format!("http://{}:{}", config.web.host, port.actual);
    let spec = build_web_start_command_spec(&repo_root, dev, &config.web.host, port.actual)?;
    if foreground {
        println!("{} starting kanban web UI: {url}", theme.ok_label());
        if open && let Err(error) = open_browser_url(&url) {
            eprintln!("{} could not open browser: {error}", theme.warning_label());
        }
        let status = process_from_spec(&spec)
            .env("KANBAN_WEB_PORT", port.actual.to_string())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("start web server with {}", spec.program))?;
        if !status.success() {
            bail!("web server exited with status {status}");
        }
        return Ok(());
    }

    write_web_port_file(&paths, port.actual)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
        .with_context(|| format!("open web log {}", paths.log_file.display()))?;
    let stderr = log
        .try_clone()
        .with_context(|| format!("clone web log handle {}", paths.log_file.display()))?;
    let mut command = process_from_spec(&spec);
    command
        .env("KANBAN_WEB_PORT", port.actual.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    command.process_group(0);
    let child = command
        .spawn()
        .with_context(|| format!("start web server with {}", spec.program))?;
    fs::write(&paths.pid_file, format!("{}\n", child.id()))
        .with_context(|| format!("write PID file {}", paths.pid_file.display()))?;

    println!("{} started kanban web UI: {url}", theme.ok_label());
    println!("{} pid: {}", theme.info_label(), child.id());
    println!(
        "{} log: {}",
        theme.info_label(),
        theme.path(paths.log_file.display())
    );
    if open && let Err(error) = open_browser_url(&url) {
        eprintln!("{} could not open browser: {error}", theme.warning_label());
    }
    Ok(())
}

pub(crate) fn stop_web(theme: &Theme, repo_root: &Path, quiet: bool) -> Result<bool> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    match read_web_process_state(&paths)? {
        WebProcessState::Stopped => {
            if !quiet {
                println!("{} web UI is not running.", theme.info_label());
            }
            Ok(false)
        }
        WebProcessState::Stale(pid) => {
            remove_pid_file(&paths)?;
            if !quiet {
                match pid {
                    Some(pid) => println!("{} removed stale PID {pid}", theme.warning_label()),
                    None => println!("{} removed stale web PID file.", theme.warning_label()),
                }
            }
            Ok(false)
        }
        WebProcessState::Running(pid) => {
            terminate_process(pid)?;
            if wait_for_process_exit(pid, 30, Duration::from_millis(100)) {
                return finish_stopped_web_process(theme, &paths, pid, quiet);
            }

            force_kill_process(pid)?;
            if wait_for_process_exit(pid, 10, Duration::from_millis(100)) {
                return finish_stopped_web_process(theme, &paths, pid, quiet);
            }

            bail!("web process {pid} did not stop after SIGTERM or SIGKILL");
        }
    }
}

pub(crate) fn print_web_status(theme: &Theme, repo_root: &Path) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    let process_state = read_web_process_state(&paths)?;
    let status_port = match process_state {
        WebProcessState::Running(_) => read_web_port_file(&paths).unwrap_or(config.web.port),
        WebProcessState::Stopped | WebProcessState::Stale(_) => config.web.port,
    };
    let url = format!("http://{}:{}", config.web.host, status_port);
    match process_state {
        WebProcessState::Running(pid) => {
            println!("{} web UI: running", theme.ok_label());
            println!("{} pid: {pid}", theme.info_label());
            println!("{} url: {url}", theme.info_label());
            println!(
                "{} log: {}",
                theme.info_label(),
                theme.path(paths.log_file.display())
            );
        }
        WebProcessState::Stopped => {
            println!("{} web UI: stopped", theme.info_label());
            println!("{} url: {url}", theme.info_label());
        }
        WebProcessState::Stale(pid) => {
            match pid {
                Some(pid) => println!("{} web UI: stale PID {pid}", theme.warning_label()),
                None => println!("{} web UI: stale PID file", theme.warning_label()),
            }
            println!(
                "{} pid file: {}",
                theme.info_label(),
                theme.path(paths.pid_file.display())
            );
        }
    }
    Ok(())
}

pub(crate) fn render_web_already_running_error(theme: &Theme, pid: u32, width: usize) -> String {
    let prefix = "✖ error";
    let prefix_width = display_width(prefix) + 1;
    let content_width = width.saturating_sub(prefix_width).max(1);
    let mut output = String::new();
    let primary = format!("kanban web is already running with PID {pid}.");
    let guidance = [
        InlineToken::plain("Use", false),
        InlineToken::command("kanban web status", true),
        InlineToken::plain("or", true),
        InlineToken::command("kanban web restart", true),
        InlineToken::plain(".", false),
    ];

    for (index, line) in wrap_text(&primary, content_width).iter().enumerate() {
        if index == 0 {
            push_line(&mut output, &format!("{} {line}", theme.error_label()));
        } else {
            push_line(&mut output, &format!("{}{line}", " ".repeat(prefix_width)));
        }
    }
    push_wrapped_inline_message(&mut output, theme, prefix_width, content_width, &guidance);

    output
}

pub(crate) fn render_web_port_fallback_warning(
    theme: &Theme,
    host: &str,
    requested_port: u16,
    actual_port: u16,
) -> String {
    format!(
        "{} another service is already using http://{}:{}; starting kanban web UI on http://{}:{} instead.",
        theme.warning_label(),
        host,
        requested_port,
        host,
        actual_port
    )
}

pub(crate) fn print_log_tail(content: &str, lines: Option<usize>) {
    match lines {
        Some(0) => {}
        Some(limit) => {
            let selected = content.lines().rev().take(limit).collect::<Vec<_>>();
            for line in selected.iter().rev() {
                println!("{line}");
            }
        }
        None => print!("{content}"),
    }
}

pub(crate) fn print_web_log(
    theme: &Theme,
    repo_root: &Path,
    lines: Option<usize>,
    follow: bool,
) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    if !paths.log_file.exists() {
        println!(
            "{} no web log found: {}",
            theme.warning_label(),
            theme.path(paths.log_file.display())
        );
        return Ok(());
    }

    let content = fs::read_to_string(&paths.log_file)
        .with_context(|| format!("read web log {}", paths.log_file.display()))?;
    print_log_tail(&content, lines);
    if !follow {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .read(true)
        .open(&paths.log_file)
        .with_context(|| format!("open web log {}", paths.log_file.display()))?;
    file.seek(SeekFrom::End(0))?;
    loop {
        let mut appended = String::new();
        file.read_to_string(&mut appended)?;
        if !appended.is_empty() {
            print!("{appended}");
            std::io::stdout().flush()?;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::process::CommandExt;

    #[test]
    fn web_runtime_paths_live_under_kanban_run() {
        let paths = web_runtime_paths(Path::new("/tmp/repo"));

        assert_eq!(paths.run_dir, PathBuf::from("/tmp/repo/.kanban/run"));
        assert_eq!(
            paths.pid_file,
            PathBuf::from("/tmp/repo/.kanban/run/web.pid")
        );
        assert_eq!(
            paths.port_file,
            PathBuf::from("/tmp/repo/.kanban/run/web.port")
        );
        assert_eq!(
            paths.log_file,
            PathBuf::from("/tmp/repo/.kanban/run/web.log")
        );
    }

    #[test]
    fn web_already_running_error_uses_icon_and_aligned_guidance() {
        let output = render_web_already_running_error(&Theme::plain(), 77322, 100);

        assert_eq!(
            output,
            "✖ error kanban web is already running with PID 77322.\n        Use kanban web status or kanban web restart.\n"
        );
    }

    #[test]
    fn web_already_running_error_wraps_with_hanging_indent() {
        let output = render_web_already_running_error(&Theme::plain(), 77322, 48);

        for line in output.lines().skip(1) {
            assert!(
                line.starts_with("        "),
                "line was not indented: {line}"
            );
        }
        assert!(output.contains("\n        77322.\n"));
        assert!(output.contains("\n        kanban web restart.\n"));
    }

    #[test]
    fn web_already_running_error_uses_theme_colors_for_error_and_commands() {
        let output = render_web_already_running_error(&Theme::color(), 77322, 100);

        assert!(output.contains("\x1b[1;31m✖ error\x1b[0m"));
        assert!(output.contains("\x1b[1;34mkanban web status\x1b[0m"));
        assert!(output.contains("\x1b[1;34mkanban web restart\x1b[0m"));
    }

    #[test]
    fn web_port_fallback_warning_reports_actual_url() {
        let output = render_web_port_fallback_warning(&Theme::plain(), "127.0.0.1", 3000, 3001);

        assert_eq!(
            output,
            "▲ warning another service is already using http://127.0.0.1:3000; starting kanban web UI on http://127.0.0.1:3001 instead."
        );
    }

    #[test]
    fn production_web_start_spec_does_not_require_source_checkout() {
        let production = build_web_start_command_spec(
            Path::new("/tmp/backlog-only-repo"),
            false,
            "127.0.0.1",
            3000,
        )
        .unwrap();
        assert!(!production.program.is_empty());
        assert_eq!(production.cwd, PathBuf::from("/tmp/backlog-only-repo"));
        assert_eq!(
            production.args,
            [
                "web",
                "serve",
                "--repo-root",
                "/tmp/backlog-only-repo",
                "--host",
                "127.0.0.1",
                "--port",
                "3000",
            ]
        );
    }

    #[test]
    fn web_start_specs_select_dev_command() {
        let repo_root = Path::new("/tmp/repo");

        let dev = build_web_start_command_spec(repo_root, true, "127.0.0.1", 3000).unwrap();
        #[cfg(windows)]
        assert_eq!(dev.program, "npm.cmd");
        #[cfg(not(windows))]
        assert_eq!(dev.program, "npm");
        assert_eq!(dev.cwd, PathBuf::from("/tmp/repo"));
        assert_eq!(dev.args[0], "--prefix");
        assert!(dev.args[1].replace('\\', "/").ends_with("/web"));
        assert_eq!(
            &dev.args[2..],
            ["run", "dev", "--", "--host", "127.0.0.1", "--port", "3000"]
        );

        let build = build_web_build_command_spec(repo_root).unwrap();
        #[cfg(windows)]
        assert_eq!(build.program, "npm.cmd");
        #[cfg(not(windows))]
        assert_eq!(build.program, "npm");
    }

    #[cfg(unix)]
    #[test]
    fn terminate_process_stops_process_group() {
        let mut child = ProcessCommand::new("sh");
        child.arg("-c").arg("sleep 30").process_group(0);
        let mut child = child.spawn().expect("spawn child process");
        let pid = child.id();

        terminate_process(pid).expect("send SIGTERM");

        assert!(
            wait_for_process_exit(pid, 30, Duration::from_millis(100)),
            "process group should exit after SIGTERM"
        );
        let _ = child.wait();
    }

    #[cfg(unix)]
    #[test]
    fn force_kill_process_stops_term_resistant_process_group() {
        let mut child = ProcessCommand::new("sh");
        child.arg("-c").arg("sleep 30").process_group(0);
        let mut child = child.spawn().expect("spawn child process");
        let pid = child.id();

        force_kill_process(pid).expect("send SIGKILL");
        assert!(
            wait_for_process_exit(pid, 10, Duration::from_millis(100)),
            "process group should exit after SIGKILL"
        );
        let _ = child.wait();
    }

    #[cfg(windows)]
    #[test]
    fn child_process_path_removes_extended_windows_prefix() {
        assert_eq!(
            child_process_path(Path::new(r"\\?\C:\repo\tools\kanban\web")),
            PathBuf::from(r"C:\repo\tools\kanban\web")
        );
        assert_eq!(
            child_process_path(Path::new(r"\\?\UNC\server\share\repo")),
            PathBuf::from(r"\\server\share\repo")
        );
    }

    #[test]
    fn process_is_kanban_web_rejects_pid_zero() {
        assert!(!process_is_kanban_web(0));
    }

    #[cfg(unix)]
    #[test]
    fn process_is_kanban_web_rejects_non_kanban_process() {
        // Spawn a `sleep` process whose command name is definitely not
        // "kanban", so we can assert the identity check rejects it (US-015
        // scenario 2: recycled PID).
        let mut child = ProcessCommand::new("sleep").arg("2").spawn().unwrap();
        let pid = child.id();
        assert!(
            !process_is_kanban_web(pid),
            "sleep process must not be identified as kanban web"
        );
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(unix)]
    #[test]
    fn read_web_process_state_treats_recycled_pid_as_stale() {
        // Write a PID file pointing at a non-kanban process and verify
        // read_web_process_state returns Stale (not Running) so stop_web
        // removes the file without signalling (US-015 scenario 2).
        let temp = tempfile::tempdir().unwrap();
        let run_dir = temp.path().join(".kanban/run");
        fs::create_dir_all(&run_dir).unwrap();
        let paths = WebRuntimePaths {
            run_dir: run_dir.clone(),
            pid_file: run_dir.join("web.pid"),
            port_file: run_dir.join("web.port"),
            log_file: run_dir.join("web.log"),
        };
        let mut child = ProcessCommand::new("sleep").arg("2").spawn().unwrap();
        fs::write(&paths.pid_file, format!("{}\n", child.id())).unwrap();

        let state = read_web_process_state(&paths).unwrap();
        assert!(
            matches!(state, WebProcessState::Stale(Some(_))),
            "recycled PID must be Stale, not Running, got {state:?}"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(unix)]
    #[test]
    fn read_web_process_state_treats_dead_pid_as_stale() {
        // Write a PID file pointing at a PID that has already exited and
        // verify read_web_process_state returns Stale (US-015 scenario 3).
        let temp = tempfile::tempdir().unwrap();
        let run_dir = temp.path().join(".kanban/run");
        fs::create_dir_all(&run_dir).unwrap();
        let paths = WebRuntimePaths {
            run_dir: run_dir.clone(),
            pid_file: run_dir.join("web.pid"),
            port_file: run_dir.join("web.port"),
            log_file: run_dir.join("web.log"),
        };
        let mut child = ProcessCommand::new("true").spawn().unwrap();
        let pid = child.id();
        let _ = child.wait();
        fs::write(&paths.pid_file, format!("{pid}\n")).unwrap();

        let state = read_web_process_state(&paths).unwrap();
        assert!(
            matches!(state, WebProcessState::Stale(Some(_))),
            "dead PID must be Stale, got {state:?}"
        );
    }
}
