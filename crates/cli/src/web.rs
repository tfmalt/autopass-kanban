#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
};
#[allow(unused_imports)]
use kanban_core::*;

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

    if !dev && !web_production_entry(&repo_root).is_file() {
        bail!(
            "built web server not found at {}. Run `kanban web start --build` or use `kanban web start --dev`.",
            web_production_entry(&repo_root).display()
        );
    }

    let port = resolve_web_port(&config.web.host, config.web.port)?;
    let url = format!("http://{}:{}", config.web.host, port.actual);
    let spec = build_web_start_command_spec(&repo_root, dev);
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebProcessState {
    Stopped,
    Running(u32),
    Stale(Option<u32>),
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

pub(crate) fn web_app_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tools/kanban-web")
}

pub(crate) fn web_production_entry(repo_root: &Path) -> PathBuf {
    web_app_dir(repo_root).join("dist/server/index.js")
}

pub(crate) fn build_web_start_command_spec(repo_root: &Path, dev: bool) -> WebStartCommandSpec {
    let web_dir = web_app_dir(repo_root);
    if dev {
        WebStartCommandSpec {
            program: "npm".to_string(),
            args: vec![
                "--prefix".to_string(),
                web_dir.to_string_lossy().into_owned(),
                "run".to_string(),
                "dev:server".to_string(),
            ],
            cwd: repo_root.to_path_buf(),
        }
    } else {
        WebStartCommandSpec {
            program: "node".to_string(),
            args: vec![
                web_production_entry(repo_root)
                    .to_string_lossy()
                    .into_owned(),
            ],
            cwd: repo_root.to_path_buf(),
        }
    }
}

pub(crate) fn build_web_build_command_spec(repo_root: &Path) -> WebStartCommandSpec {
    WebStartCommandSpec {
        program: "npm".to_string(),
        args: vec![
            "--prefix".to_string(),
            web_app_dir(repo_root).to_string_lossy().into_owned(),
            "run".to_string(),
            "build".to_string(),
        ],
        cwd: repo_root.to_path_buf(),
    }
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

pub(crate) fn read_web_process_state(paths: &WebRuntimePaths) -> Result<WebProcessState> {
    if !paths.pid_file.exists() {
        return Ok(WebProcessState::Stopped);
    }

    let raw = fs::read_to_string(&paths.pid_file)
        .with_context(|| format!("read web PID file {}", paths.pid_file.display()))?;
    let trimmed = raw.trim();
    let Ok(pid) = trimmed.parse::<u32>() else {
        return Ok(WebProcessState::Stale(None));
    };
    if pid == 0 {
        return Ok(WebProcessState::Stale(None));
    }

    if process_exists(pid) {
        Ok(WebProcessState::Running(pid))
    } else {
        Ok(WebProcessState::Stale(Some(pid)))
    }
}

#[cfg(unix)]
pub(crate) fn process_exists(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
pub(crate) fn process_exists(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn terminate_process(pid: u32) -> Result<()> {
    let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if result == 0 || !process_exists(pid) {
        Ok(())
    } else {
        bail!("failed to stop web process {pid}");
    }
}

#[cfg(not(unix))]
pub(crate) fn terminate_process(_pid: u32) -> Result<()> {
    bail!("kanban web stop is only implemented on Unix-like systems.")
}

pub(crate) fn remove_pid_file(paths: &WebRuntimePaths) -> Result<()> {
    if paths.pid_file.exists() {
        fs::remove_file(&paths.pid_file)
            .with_context(|| format!("remove PID file {}", paths.pid_file.display()))?;
    }
    if paths.port_file.exists() {
        fs::remove_file(&paths.port_file)
            .with_context(|| format!("remove web port file {}", paths.port_file.display()))?;
    }
    Ok(())
}

pub(crate) fn read_web_port_file(paths: &WebRuntimePaths) -> Option<u16> {
    fs::read_to_string(&paths.port_file)
        .ok()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .filter(|port| *port != 0)
}

pub(crate) fn write_web_port_file(paths: &WebRuntimePaths, port: u16) -> Result<()> {
    fs::write(&paths.port_file, format!("{port}\n"))
        .with_context(|| format!("write web port file {}", paths.port_file.display()))
}

pub(crate) fn run_web_build(repo_root: &Path) -> Result<()> {
    let spec = build_web_build_command_spec(repo_root);
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
        println!("{}", theme.label("Building kanban web UI..."));
        run_web_build(&repo_root)?;
    }
    if !dev && !web_production_entry(&repo_root).is_file() {
        bail!(
            "built web server not found at {}. Run `kanban web start --build` or use `kanban web start --dev`.",
            web_production_entry(&repo_root).display()
        );
    }

    let port = resolve_web_port(&config.web.host, config.web.port)?;
    if port.changed() {
        println!(
            "{}",
            render_web_port_fallback_warning(theme, &config.web.host, port.requested, port.actual)
        );
    }

    let url = format!("http://{}:{}", config.web.host, port.actual);
    let spec = build_web_start_command_spec(&repo_root, dev);
    if foreground {
        println!("{} {url}", theme.success("Starting kanban web UI:"));
        if open && let Err(error) = open_browser_url(&url) {
            eprintln!("{} {error}", theme.warning("Could not open browser:"));
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

    println!("{} {url}", theme.success("Started kanban web UI:"));
    println!("{} {}", theme.label("PID:"), child.id());
    println!(
        "{} {}",
        theme.label("Log:"),
        theme.path(paths.log_file.display())
    );
    if open && let Err(error) = open_browser_url(&url) {
        eprintln!("{} {error}", theme.warning("Could not open browser:"));
    }
    Ok(())
}

pub(crate) fn stop_web(theme: &Theme, repo_root: &Path, quiet: bool) -> Result<bool> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    match read_web_process_state(&paths)? {
        WebProcessState::Stopped => {
            if !quiet {
                println!("{}", theme.warning("kanban web UI is not running."));
            }
            Ok(false)
        }
        WebProcessState::Stale(pid) => {
            remove_pid_file(&paths)?;
            if !quiet {
                match pid {
                    Some(pid) => println!("{} stale PID {pid}", theme.warning("Removed")),
                    None => println!("{}", theme.warning("Removed stale web PID file.")),
                }
            }
            Ok(false)
        }
        WebProcessState::Running(pid) => {
            terminate_process(pid)?;
            for _ in 0..30 {
                if !process_exists(pid) {
                    remove_pid_file(&paths)?;
                    if !quiet {
                        println!("{} PID {pid}", theme.success("Stopped kanban web UI:"));
                    }
                    return Ok(true);
                }
                thread::sleep(Duration::from_millis(100));
            }
            bail!("web process {pid} did not stop after SIGTERM");
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
            println!("{} running", theme.success("kanban web UI:"));
            println!("{} {pid}", theme.label("PID:"));
            println!("{} {url}", theme.label("URL:"));
            println!(
                "{} {}",
                theme.label("Log:"),
                theme.path(paths.log_file.display())
            );
        }
        WebProcessState::Stopped => {
            println!("{} stopped", theme.warning("kanban web UI:"));
            println!("{} {url}", theme.label("URL:"));
        }
        WebProcessState::Stale(pid) => {
            match pid {
                Some(pid) => println!("{} stale PID {pid}", theme.warning("kanban web UI:")),
                None => println!("{} stale PID file", theme.warning("kanban web UI:")),
            }
            println!(
                "{} {}",
                theme.label("PID file:"),
                theme.path(paths.pid_file.display())
            );
        }
    }
    Ok(())
}

pub(crate) fn render_web_already_running_error(theme: &Theme, pid: u32, width: usize) -> String {
    let icon = "✖";
    let prefix_width = display_width(icon) + 1;
    let content_width = width.saturating_sub(prefix_width).max(1);
    let mut output = String::new();
    let primary = format!("Error: kanban web is already running with PID {pid}.");
    let guidance = [
        InlineToken::plain("Use", false),
        InlineToken::command("`kanban web status`", true),
        InlineToken::plain("or", true),
        InlineToken::command("`kanban web restart`", true),
        InlineToken::plain(".", false),
    ];

    for (index, line) in wrap_text(&primary, content_width).iter().enumerate() {
        if index == 0 {
            if let Some(rest) = line.strip_prefix("Error:") {
                push_line(
                    &mut output,
                    &format!("{} {}{}", theme.error(icon), theme.error("Error:"), rest),
                );
            } else {
                push_line(&mut output, &format!("{} {line}", theme.error(icon)));
            }
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
        theme.warning("Warning:"),
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
            "{} {}",
            theme.warning("No web log found:"),
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
