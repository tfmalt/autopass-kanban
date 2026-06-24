#[allow(unused_imports)]
use crate::prelude::*;
use crate::theme::Theme;
use kanban_core::ColorMode;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

const RAW_GITHUB_HOST: &str = "https://raw.githubusercontent.com/tfmalt/autopass-kanban";
const GITHUB_API_BASE: &str = "https://api.github.com/repos/tfmalt/autopass-kanban";
const INSTALL_SCRIPT_PATH: &str = "scripts/install.sh";
const UNINSTALL_SCRIPT: &str = include_str!("../../../scripts/uninstall.sh");

/// Env var that explicitly opts in to the unsafe `GITHUB_LATEST_TAG` /
/// `GITHUB_API_BASE` overrides outside test configuration. Documents the trust
/// implications to operators who need them (e.g. air-gapped mirrors).
const ALLOW_UNSAFE_OVERRIDE_ENV: &str = "KANBAN_ALLOW_UNSAFE_OVERRIDE";

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
    let theme = Theme::for_stdout(ColorMode::Auto);
    let latest_version = resolve_latest_version()?;
    run_upgrade_with_latest(&theme, options, &latest_version)
}

#[cfg(not(windows))]
fn run_upgrade_with_latest(
    theme: &Theme,
    options: UpgradeOptions,
    latest_version: &str,
) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    if !is_newer_version(current_version, latest_version)? {
        println!(
            "{} {} is the latest version",
            theme.brand(),
            theme.version(current_version)
        );
        return Ok(());
    }

    let tag = expected_tag_for_version(latest_version);
    ensure_pinned_tag(&tag)?;
    let script_url = pinned_install_script_url(&tag);
    let checksum_url = format!("{}.sha256", script_url);

    if !options.quiet {
        println!("{} fetching pinned install script for {tag}", theme.brand());
    }
    let script_bytes = download_bytes(&script_url)
        .with_context(|| format!("download install script {script_url}"))?;
    let checksum_bytes = download_bytes(&checksum_url)
        .with_context(|| format!("download install script checksum {checksum_url}"))?;
    let expected_hex = parse_checksum_asset(&checksum_bytes)
        .context("install script checksum asset is malformed")?;
    verify_sha256(&script_bytes, &expected_hex)
        .context("install script checksum verification failed")?;

    let install_path = temp_script_path("kanban-install");
    fs::write(&install_path, &script_bytes)
        .with_context(|| format!("write install script {}", install_path.display()))?;
    let status = ProcessCommand::new("sh")
        .arg(&install_path)
        .args(upgrade_args(&options, Some(latest_version)))
        .status();
    let _ = fs::remove_file(&install_path);
    let status = status.context("failed to run verified kanban installer")?;

    if !status.success() {
        bail!("kanban upgrade failed with status {status}");
    }

    Ok(())
}

/// Build the install script URL pinned to a specific release tag.
/// `tag` must already be the `v<version>` form; `main` is hard-refused by the
/// caller.
pub(crate) fn pinned_install_script_url(tag: &str) -> String {
    format!("{RAW_GITHUB_HOST}/{tag}/{INSTALL_SCRIPT_PATH}")
}

/// The release tag form kanban upgrade pins to (`v<version>`).
pub(crate) fn expected_tag_for_version(version: &str) -> String {
    let normalized = normalize_version(version);
    if normalized == "main" {
        "main".to_string()
    } else {
        format!("v{normalized}")
    }
}

/// Hard-refuse to fetch the install script from `main` or an empty tag.
/// `kanban upgrade` only executes scripts pinned to a release tag (US-010).
pub(crate) fn ensure_pinned_tag(tag: &str) -> Result<()> {
    if tag == "main" || tag.is_empty() {
        bail!(
            "Refusing to fetch the install script from `{tag}`. kanban upgrade only runs scripts pinned to a release tag."
        );
    }
    Ok(())
}

/// Parse a `.sha256` checksum asset. Accepts `<hex>`, `<hex>  install.sh`,
/// and `<hex> *install.sh` forms (the formats `sha256sum` produces).
pub(crate) fn parse_checksum_asset(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes).with_context(|| "checksum asset is not valid UTF-8")?;
    let hex = text
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            // `sha256sum` output is `<hex> <mode><filename>`; the hex is first.
            trimmed.split_whitespace().next()
        })
        .context("checksum asset is empty")?;
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("checksum asset does not contain a 64-character hex SHA-256");
    }
    Ok(hex.to_lowercase())
}

/// Verify the SHA-256 of `script` against the expected lowercase hex digest.
pub(crate) fn verify_sha256(script: &[u8], expected_hex: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(script);
    let actual = hasher.finalize();
    let actual_hex: String = actual.iter().map(|b| format!("{b:02x}")).collect();
    if actual_hex != expected_hex.to_lowercase() {
        bail!("install script checksum mismatch: expected {expected_hex}, computed {actual_hex}");
    }
    Ok(())
}

pub(crate) fn latest_version_if_newer() -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    let latest = resolve_latest_version().ok()?;
    match is_newer_version(current, &latest) {
        Ok(true) => Some(latest),
        _ => None,
    }
}

#[cfg(windows)]
fn resolve_latest_version() -> Result<String> {
    bail!("kanban upgrade is only supported on POSIX shells")
}

#[cfg(not(windows))]
fn resolve_latest_version() -> Result<String> {
    // `GITHUB_LATEST_TAG` and `GITHUB_API_BASE` can redirect or suppress the
    // update check and are therefore unsafe in production. They are honored
    // only under test configuration or when an operator explicitly opts in via
    // `KANBAN_ALLOW_UNSAFE_OVERRIDE=1` (US-010 scenario 3).
    if unsafe_override_enabled()
        && let Ok(tag) = std::env::var("GITHUB_LATEST_TAG")
    {
        return Ok(normalize_version(&tag));
    }

    let api_base = if unsafe_override_enabled() {
        std::env::var("GITHUB_API_BASE").unwrap_or_else(|_| GITHUB_API_BASE.to_string())
    } else {
        GITHUB_API_BASE.to_string()
    };
    let body = download_bytes(&format!("{api_base}/releases/latest"))?;
    let json: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse latest GitHub release metadata")?;
    let tag = json
        .get("tag_name")
        .and_then(|value| value.as_str())
        .context("latest GitHub release metadata did not include tag_name")?;
    Ok(normalize_version(tag))
}

/// Whether the unsafe upgrade-flow env overrides are honored.
fn unsafe_override_enabled() -> bool {
    cfg!(test) || std::env::var(ALLOW_UNSAFE_OVERRIDE_ENV).as_deref() == Ok("1")
}

#[cfg(not(windows))]
fn download_bytes(url: &str) -> Result<Vec<u8>> {
    if let Some(output) = run_downloader("curl", &["-fsSL", url])? {
        return Ok(output);
    }
    if let Some(output) = run_downloader("wget", &["-qO-", url])? {
        return Ok(output);
    }
    bail!("kanban upgrade requires curl or wget to download release artifacts")
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
            &Theme::color(),
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

    #[test]
    fn latest_version_if_newer_respects_github_latest_tag_env() {
        // The GITHUB_LATEST_TAG env var short-circuits resolve_latest_version,
        // so we can exercise latest_version_if_newer without network access.
        let current = env!("CARGO_PKG_VERSION");
        let parsed = parse_version(current).unwrap();
        let newer = format!("{}.{}.{}", parsed.0, parsed.1, parsed.2 + 1);

        // SAFETY: no other test touches GITHUB_LATEST_TAG, so there is no
        // concurrent mutation risk.
        unsafe {
            std::env::set_var("GITHUB_LATEST_TAG", format!("v{newer}"));
        }
        let result = latest_version_if_newer();
        unsafe {
            std::env::remove_var("GITHUB_LATEST_TAG");
        }

        assert_eq!(result.as_deref(), Some(newer.as_str()));
    }

    #[test]
    fn latest_version_if_newer_returns_none_when_current_is_latest() {
        // SAFETY: no other test touches GITHUB_LATEST_TAG, so there is no
        // concurrent mutation risk.
        unsafe {
            std::env::set_var("GITHUB_LATEST_TAG", env!("CARGO_PKG_VERSION"));
        }
        let result = latest_version_if_newer();
        unsafe {
            std::env::remove_var("GITHUB_LATEST_TAG");
        }

        assert!(result.is_none());
    }

    #[test]
    fn pinned_install_script_url_uses_tag_not_main() {
        let url = pinned_install_script_url("v26.6.2401");
        assert!(
            url.contains("/v26.6.2401/scripts/install.sh"),
            "expected pinned tag URL, got {url}"
        );
        assert!(
            !url.contains("/main/"),
            "pinned URL must not reference main"
        );
    }

    #[test]
    fn expected_tag_for_version_prefixes_v() {
        assert_eq!(expected_tag_for_version("26.6.2401"), "v26.6.2401");
        assert_eq!(expected_tag_for_version("v26.6.2401"), "v26.6.2401");
        assert_eq!(expected_tag_for_version("main"), "main");
    }

    #[test]
    fn parse_checksum_asset_accepts_sha256sum_forms() {
        // `sha256sum install.sh > install.sh.sha256` form.
        let a = b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  install.sh\n";
        assert_eq!(
            parse_checksum_asset(a).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // Binary mode marker and bare hex form.
        let b = b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 *install.sh\n";
        assert_eq!(
            parse_checksum_asset(b).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let bare = b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(
            parse_checksum_asset(bare).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn parse_checksum_asset_rejects_malformed_hex() {
        assert!(parse_checksum_asset(b"tooshort").is_err());
        assert!(parse_checksum_asset(b"zz55".repeat(16).as_slice()).is_err());
        assert!(parse_checksum_asset(b"").is_err());
    }

    #[test]
    fn verify_sha256_accepts_match_and_rejects_mismatch() {
        let script = b"#!/bin/sh\necho hello\n";
        let mut hasher = sha2::Sha256::new();
        sha2::Digest::update(&mut hasher, script);
        let digest = sha2::Digest::finalize(hasher);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();

        verify_sha256(script, &hex).expect("matching checksum verifies");

        let err = verify_sha256(script, "deadbeef").unwrap_err();
        assert!(
            err.to_string().contains("checksum mismatch"),
            "expected mismatch error, got {err}"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn ensure_pinned_tag_refuses_main_and_empty() {
        assert!(ensure_pinned_tag("main").is_err());
        assert!(ensure_pinned_tag("").is_err());
        ensure_pinned_tag("v26.6.2401").expect("real tag is accepted");
    }
}
