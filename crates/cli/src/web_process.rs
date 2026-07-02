use anyhow::{Context, Result, bail};
use std::fs;
use std::process::Command as ProcessCommand;
use std::thread;
use std::time::Duration;

use crate::theme::Theme;
use crate::web::WebRuntimePaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebProcessState {
    Stopped,
    Running(u32),
    Stale(Option<u32>),
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

    if process_exists(pid) && process_is_kanban_web(pid) {
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

/// Verify that the process at `pid` is a `kanban` process before signalling it
/// (US-015). On Unix, `ps -o comm= -p {pid}` returns the command name; we
/// require it to contain `kanban` so a recycled PID owned by an unrelated
/// process is not signalled.
#[cfg(unix)]
pub(crate) fn process_is_kanban_web(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let output = ProcessCommand::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let comm = String::from_utf8_lossy(&output.stdout);
            comm.trim().contains("kanban")
        }
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

/// Verify that the process at `pid` is a `kanban` process before signalling it
/// (US-015). On Windows, query the full process image name and require the
/// executable stem to be `kanban`.
#[cfg(windows)]
pub(crate) fn process_is_kanban_web(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len) != 0 };
    unsafe {
        let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
    }
    if !ok {
        return false;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    let exe_name = path.rsplit(['\\', '/']).next().unwrap_or(&path);
    exe_name.to_ascii_lowercase().starts_with("kanban")
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn process_is_kanban_web(_pid: u32) -> bool {
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

pub(crate) fn wait_for_process_exit(pid: u32, attempts: usize, pause: Duration) -> bool {
    for _ in 0..attempts {
        if !process_exists(pid) {
            return true;
        }
        thread::sleep(pause);
    }
    !process_exists(pid)
}

pub(crate) fn finish_stopped_web_process(
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
