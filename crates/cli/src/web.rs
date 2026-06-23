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
    let web_dir = child_process_path(&web_app_dir(repo_root)?);
    let cwd = child_process_path(repo_root);
    if dev {
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
    if pid == 0 {
        return false;
    }

    (unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }) && !process_is_zombie(pid)
}

#[cfg(unix)]
fn process_is_zombie(pid: u32) -> bool {
    let output = ProcessCommand::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .is_some_and(|stat| stat.starts_with('Z')),
        _ => false,
    }
}

#[cfg(windows)]
pub(crate) fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    let handle = unsafe {
        windows_sys::Win32::System::Threading::OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return false;
    }

    let mut exit_code = 0;
    let is_running = unsafe {
        windows_sys::Win32::System::Threading::GetExitCodeProcess(handle, &mut exit_code) != 0
            && exit_code == windows_sys::Win32::Foundation::STILL_ACTIVE as u32
    };
    unsafe {
        let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
    }
    is_running
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn process_exists(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn terminate_process(pid: u32) -> Result<()> {
    send_signal_to_process(pid, libc::SIGTERM, "SIGTERM")
}

#[cfg(unix)]
pub(crate) fn force_kill_process(pid: u32) -> Result<()> {
    send_signal_to_process(pid, libc::SIGKILL, "SIGKILL")
}

#[cfg(unix)]
fn send_signal_to_process(pid: u32, signal: libc::c_int, signal_name: &str) -> Result<()> {
    if pid == 0 {
        return Ok(());
    }

    // `kanban web start` creates a dedicated process group so a stop request
    // tears down the full web tree in both production and dev mode.
    let process_group_result = unsafe { libc::kill(-(pid as libc::pid_t), signal) };
    if process_group_result == 0 || !process_exists(pid) {
        return Ok(());
    }

    let process_result = unsafe { libc::kill(pid as libc::pid_t, signal) };
    if process_result == 0 || !process_exists(pid) {
        Ok(())
    } else {
        bail!("failed to send {signal_name} to web process {pid}");
    }
}

#[cfg(not(unix))]
#[cfg(not(windows))]
pub(crate) fn terminate_process(_pid: u32) -> Result<()> {
    bail!("kanban web stop is not implemented on this platform.")
}

#[cfg(not(unix))]
#[cfg(not(windows))]
pub(crate) fn force_kill_process(_pid: u32) -> Result<()> {
    bail!("kanban web stop is not implemented on this platform.")
}

#[cfg(windows)]
pub(crate) fn terminate_process(pid: u32) -> Result<()> {
    if !process_exists(pid) {
        return Ok(());
    }

    let handle = unsafe {
        windows_sys::Win32::System::Threading::OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_TERMINATE,
            0,
            pid,
        )
    };
    if handle.is_null() {
        bail!("failed to open web process {pid} for termination");
    }

    let terminated =
        unsafe { windows_sys::Win32::System::Threading::TerminateProcess(handle, 0) != 0 };
    unsafe {
        let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
    }
    if terminated || !process_exists(pid) {
        Ok(())
    } else {
        bail!("failed to stop web process {pid}")
    }
}

#[cfg(windows)]
pub(crate) fn force_kill_process(pid: u32) -> Result<()> {
    terminate_process(pid)
}

fn wait_for_process_exit(pid: u32, attempts: usize, pause: Duration) -> bool {
    for _ in 0..attempts {
        if !process_exists(pid) {
            return true;
        }
        thread::sleep(pause);
    }
    !process_exists(pid)
}

fn finish_stopped_web_process(
    theme: &Theme,
    paths: &WebRuntimePaths,
    pid: u32,
    quiet: bool,
) -> Result<bool> {
    remove_pid_file(paths)?;
    if !quiet {
        println!("{} stopped kanban web UI: PID {pid}", theme.ok_label());
    }
    Ok(true)
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
    fn web_start_specs_select_production_or_dev_command() {
        let repo_root = Path::new("/tmp/repo");

        let production = build_web_start_command_spec(repo_root, false, "127.0.0.1", 3000).unwrap();
        assert!(!production.program.is_empty());
        assert_eq!(production.cwd, PathBuf::from("/tmp/repo"));
        assert_eq!(
            production.args,
            [
                "web",
                "serve",
                "--repo-root",
                "/tmp/repo",
                "--host",
                "127.0.0.1",
                "--port",
                "3000",
            ]
        );

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
}
