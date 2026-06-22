#[allow(unused_imports)]
use crate::prelude::*;
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

const REMOTE_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh";
const GITHUB_API_BASE: &str = "https://api.github.com/repos/tfmalt/autopass-kanban";
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
    let latest_version = resolve_latest_version()?;
    run_upgrade_with_latest(options, &latest_version)
}

#[cfg(not(windows))]
fn run_upgrade_with_latest(options: UpgradeOptions, latest_version: &str) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    if !is_newer_version(current_version, latest_version)? {
        println!(
            "kanban is already at the latest version (current: {current_version}, latest: {latest_version})."
        );
        return Ok(());
    }

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
        .args(upgrade_args(&options, Some(latest_version)))
        .status()
        .context("failed to run remote kanban installer")?;

    if !status.success() {
        bail!("kanban upgrade failed with status {status}");
    }

    Ok(())
}

#[cfg(not(windows))]
fn resolve_latest_version() -> Result<String> {
    if let Ok(tag) = std::env::var("GITHUB_LATEST_TAG") {
        return Ok(normalize_version(&tag));
    }

    let api_base = std::env::var("GITHUB_API_BASE").unwrap_or_else(|_| GITHUB_API_BASE.to_string());
    let body = download_stdout(&format!("{api_base}/releases/latest"))?;
    let json: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse latest GitHub release metadata")?;
    let tag = json
        .get("tag_name")
        .and_then(|value| value.as_str())
        .context("latest GitHub release metadata did not include tag_name")?;
    Ok(normalize_version(tag))
}

#[cfg(not(windows))]
fn download_stdout(url: &str) -> Result<Vec<u8>> {
    if let Some(output) = run_downloader("curl", &["-fsSL", url])? {
        return Ok(output);
    }
    if let Some(output) = run_downloader("wget", &["-qO-", url])? {
        return Ok(output);
    }
    bail!("kanban upgrade requires curl or wget to check the latest release")
}

#[cfg(not(windows))]
fn run_downloader(command: &str, args: &[&str]) -> Result<Option<Vec<u8>>> {
    let output = match ProcessCommand::new(command).args(args).output() {
        Ok(output) => output,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("failed to run {command}")),
    };
    if !output.status.success() {
        bail!("{command} failed while checking latest kanban release")
    }
    Ok(Some(output.stdout))
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn is_newer_version(current: &str, latest: &str) -> Result<bool> {
    Ok(parse_version(latest)? > parse_version(current)?)
}

fn parse_version(version: &str) -> Result<(u64, u64, u64)> {
    let normalized = normalize_version(version);
    let mut parts = normalized.split('.');
    let major = parse_version_part(parts.next(), version)?;
    let minor = parse_version_part(parts.next(), version)?;
    let patch = parse_version_part(parts.next(), version)?;
    if parts.next().is_some() {
        bail!("invalid version '{version}'")
    }
    Ok((major, minor, patch))
}

fn parse_version_part(part: Option<&str>, original: &str) -> Result<u64> {
    part.context("missing version component")?
        .parse::<u64>()
        .with_context(|| format!("invalid version '{original}'"))
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

pub(crate) fn upgrade_args(options: &UpgradeOptions, version: Option<&str>) -> Vec<OsString> {
    let mut args = Vec::new();
    if let Some(version) = version {
        args.push("--version".into());
        args.push(format!("v{version}").into());
    }
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
        let args = upgrade_args(
            &UpgradeOptions {
                prefix: Some(PathBuf::from("/tmp/bin")),
                skills_dir: None,
                no_skills: true,
                yes: true,
                force: true,
                dry_run: true,
                quiet: false,
            },
            Some("26.6.2208"),
        );

        assert_eq!(
            strings(args),
            [
                "--version",
                "v26.6.2208",
                "--prefix",
                "/tmp/bin",
                "--no-skills",
                "--yes",
                "--force",
                "--dry-run"
            ]
        );
    }

    #[test]
    fn version_compare_detects_newer_latest_release() {
        assert!(is_newer_version("26.6.2207", "26.6.2208").unwrap());
        assert!(is_newer_version("26.6.2207", "26.7.101").unwrap());
        assert!(is_newer_version("26.6.2207", "27.1.101").unwrap());
    }

    #[test]
    fn version_compare_rejects_equal_or_older_latest_release() {
        assert!(!is_newer_version("26.6.2207", "26.6.2207").unwrap());
        assert!(!is_newer_version("26.6.2207", "v26.6.2207").unwrap());
        assert!(!is_newer_version("26.6.2207", "26.6.2206").unwrap());
    }

    #[cfg(not(windows))]
    #[test]
    fn run_upgrade_returns_ok_without_installer_when_current_is_latest() {
        let result = run_upgrade_with_latest(
            UpgradeOptions {
                prefix: None,
                skills_dir: None,
                no_skills: true,
                yes: true,
                force: false,
                dry_run: true,
                quiet: true,
            },
            env!("CARGO_PKG_VERSION"),
        );

        result.unwrap();
    }
}
