#[allow(unused_imports)]
use crate::prelude::*;
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

const REMOTE_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh";
const UNINSTALL_SCRIPT: &str = include_str!("../../../scripts/uninstall.sh");

#[derive(Debug)]
pub(crate) struct UninstallOptions {
    pub(crate) prefix: Option<PathBuf>,
    pub(crate) skills_dir: Option<PathBuf>,
    pub(crate) yes: bool,
    pub(crate) dry_run: bool,
    pub(crate) quiet: bool,
}

#[derive(Debug)]
pub(crate) struct UpgradeOptions {
    pub(crate) prefix: Option<PathBuf>,
    pub(crate) skills_dir: Option<PathBuf>,
    pub(crate) no_skills: bool,
    pub(crate) yes: bool,
    pub(crate) force: bool,
    pub(crate) dry_run: bool,
    pub(crate) quiet: bool,
}

#[cfg(windows)]
pub(crate) fn run_uninstall(_options: UninstallOptions) -> Result<()> {
    bail!("kanban uninstall is only supported on POSIX shells")
}

#[cfg(not(windows))]
pub(crate) fn run_uninstall(options: UninstallOptions) -> Result<()> {
    let script_path = write_temp_script("kanban-uninstall", UNINSTALL_SCRIPT)?;
    let status = ProcessCommand::new("sh")
        .arg(&script_path)
        .args(uninstall_args(&options))
        .status();
    let _ = fs::remove_file(&script_path);
    let status = status.context("failed to run embedded kanban uninstaller")?;

    if !status.success() {
        bail!("kanban uninstall failed with status {status}");
    }

    Ok(())
}

#[cfg(windows)]
pub(crate) fn run_upgrade(_options: UpgradeOptions) -> Result<()> {
    bail!("kanban upgrade is only supported on POSIX shells")
}

#[cfg(not(windows))]
pub(crate) fn run_upgrade(options: UpgradeOptions) -> Result<()> {
    let install_path = temp_script_path("kanban-install");
    let status = ProcessCommand::new("sh")
        .arg("-c")
        .arg(
            r#"set -eu
_url=$1
_tmp=$2
shift 2
cleanup() { rm -f "$_tmp"; }
trap cleanup EXIT HUP INT TERM
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$_url" -o "$_tmp"
elif command -v wget >/dev/null 2>&1; then
    wget -q "$_url" -O "$_tmp"
else
    echo "kanban-upgrade: no downloader found (curl or wget required)" >&2
    exit 1
fi
sh "$_tmp" "$@"
"#,
        )
        .arg("kanban-upgrade")
        .arg(REMOTE_INSTALL_URL)
        .arg(&install_path)
        .args(upgrade_args(&options))
        .status()
        .context("failed to run remote kanban installer")?;

    if !status.success() {
        bail!("kanban upgrade failed with status {status}");
    }

    Ok(())
}

fn write_temp_script(prefix: &str, contents: &str) -> Result<PathBuf> {
    let path = temp_script_path(prefix);
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn temp_script_path(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{stamp}.sh", std::process::id()))
}

pub(crate) fn uninstall_args(options: &UninstallOptions) -> Vec<OsString> {
    let mut args = Vec::new();
    push_path_arg(&mut args, "--prefix", options.prefix.as_ref());
    push_path_arg(&mut args, "--skills-dir", options.skills_dir.as_ref());
    push_bool_arg(&mut args, "--yes", options.yes);
    push_bool_arg(&mut args, "--dry-run", options.dry_run);
    push_bool_arg(&mut args, "--quiet", options.quiet);
    args
}

pub(crate) fn upgrade_args(options: &UpgradeOptions) -> Vec<OsString> {
    let mut args = Vec::new();
    push_path_arg(&mut args, "--prefix", options.prefix.as_ref());
    push_path_arg(&mut args, "--skills-dir", options.skills_dir.as_ref());
    push_bool_arg(&mut args, "--no-skills", options.no_skills);
    push_bool_arg(&mut args, "--yes", options.yes);
    push_bool_arg(&mut args, "--force", options.force);
    push_bool_arg(&mut args, "--dry-run", options.dry_run);
    push_bool_arg(&mut args, "--quiet", options.quiet);
    args
}

fn push_path_arg(args: &mut Vec<OsString>, flag: &str, value: Option<&PathBuf>) {
    if let Some(value) = value {
        args.push(flag.into());
        args.push(value.as_os_str().to_owned());
    }
}

fn push_bool_arg(args: &mut Vec<OsString>, flag: &str, value: bool) {
    if value {
        args.push(flag.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect()
    }

    #[test]
    fn uninstall_args_mirror_uninstall_script_flags() {
        let args = uninstall_args(&UninstallOptions {
            prefix: Some(PathBuf::from("/tmp/bin")),
            skills_dir: Some(PathBuf::from("/tmp/skills")),
            yes: true,
            dry_run: true,
            quiet: true,
        });

        assert_eq!(
            strings(args),
            [
                "--prefix",
                "/tmp/bin",
                "--skills-dir",
                "/tmp/skills",
                "--yes",
                "--dry-run",
                "--quiet"
            ]
        );
    }

    #[test]
    fn upgrade_args_run_latest_remote_install_by_default() {
        let args = upgrade_args(&UpgradeOptions {
            prefix: Some(PathBuf::from("/tmp/bin")),
            skills_dir: None,
            no_skills: true,
            yes: true,
            force: true,
            dry_run: true,
            quiet: false,
        });

        assert_eq!(
            strings(args),
            [
                "--prefix",
                "/tmp/bin",
                "--no-skills",
                "--yes",
                "--force",
                "--dry-run"
            ]
        );
    }
}
