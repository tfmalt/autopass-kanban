use std::path::Path;
use std::process::{Command, Output};

use tempfile::tempdir;

fn kanban_in(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("kanban binary should run")
}

fn kanban_with_config_home(dir: &Path, config_home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(dir)
        .env("KANBAN_CONFIG_HOME", config_home)
        .args(args)
        .output()
        .expect("kanban binary should run")
}

fn git_init(dir: &Path) {
    let status = Command::new("git")
        .arg("init")
        .arg(dir)
        .status()
        .expect("git should run");
    assert!(status.success());
}

fn init_kanban_repo(dir: &Path, config_home: &Path) {
    git_init(dir);
    let output = kanban_with_config_home(dir, config_home, &["init"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bare_kanban_prints_help_and_git_requirement_outside_git() {
    let dir = tempdir().expect("temp dir should be created");

    let output = kanban_in(dir.path(), &[]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Usage: kanban"));
    assert!(stdout.contains("Git requirement:"));
    assert!(stdout.contains("Run `git init` before `kanban init`"));
}

#[test]
fn help_prints_git_requirement_outside_git() {
    let dir = tempdir().expect("temp dir should be created");

    let output = kanban_in(dir.path(), &["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Usage: kanban"));
    assert!(stdout.contains("Git requirement:"));
    assert!(stdout.contains("Most `kanban` commands must be run inside a git repository."));
}

#[test]
fn init_outside_git_reports_git_repository_requirement() {
    let dir = tempdir().expect("temp dir should be created");

    let output = kanban_in(dir.path(), &["init"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("✖ error") || stderr.contains("error"));
    assert!(stderr.contains("Current directory is not a git repository."));
    assert!(stderr.contains("Run git init to initialize it."));
}

#[test]
fn configured_default_serves_commands_from_an_unrelated_git_repository() {
    let workspace = tempdir().expect("workspace should be created");
    let config_home = workspace.path().join("config");
    let backlog = workspace.path().join("backlog");
    let service = workspace.path().join("service");
    std::fs::create_dir_all(&backlog).unwrap();
    std::fs::create_dir_all(&service).unwrap();
    init_kanban_repo(&backlog, &config_home);
    git_init(&service);

    let set = kanban_with_config_home(
        &service,
        &config_home,
        &["config", "global", "set-root", backlog.to_str().unwrap()],
    );
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );

    let output =
        kanban_with_config_home(&service, &config_home, &["config", "get", "paths.backlog"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "delivery/backlog"
    );
}

#[test]
fn explicit_current_directory_overrides_configured_default() {
    let workspace = tempdir().expect("workspace should be created");
    let config_home = workspace.path().join("config");
    let backlog = workspace.path().join("backlog");
    let local = workspace.path().join("local");
    std::fs::create_dir_all(&backlog).unwrap();
    std::fs::create_dir_all(&local).unwrap();
    init_kanban_repo(&backlog, &config_home);
    init_kanban_repo(&local, &config_home);

    let set = kanban_with_config_home(
        &local,
        &config_home,
        &["config", "global", "set-root", backlog.to_str().unwrap()],
    );
    assert!(set.status.success());
    let local_set = kanban_with_config_home(
        &local,
        &config_home,
        &["config", "set", "theme.color_mode", "always", "."],
    );
    assert!(local_set.status.success());

    let local_default =
        kanban_with_config_home(&local, &config_home, &["config", "get", "theme.color_mode"]);
    assert!(local_default.status.success());
    assert_eq!(
        String::from_utf8_lossy(&local_default.stdout).trim(),
        "always"
    );

    let output = kanban_with_config_home(
        &local,
        &config_home,
        &["config", "get", "theme.color_mode", "."],
    );
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "always");
}

#[test]
fn environment_root_overrides_local_and_configured_defaults() {
    let workspace = tempdir().expect("workspace should be created");
    let config_home = workspace.path().join("config");
    let backlog = workspace.path().join("backlog");
    let environment_backlog = workspace.path().join("environment-backlog");
    let local = workspace.path().join("local");
    for directory in [&backlog, &environment_backlog, &local] {
        std::fs::create_dir_all(directory).unwrap();
        init_kanban_repo(directory, &config_home);
    }
    let set = kanban_with_config_home(
        &local,
        &config_home,
        &["config", "global", "set-root", backlog.to_str().unwrap()],
    );
    assert!(set.status.success());
    let environment_set = kanban_with_config_home(
        &environment_backlog,
        &config_home,
        &["config", "set", "theme.color_mode", "always", "."],
    );
    assert!(environment_set.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(&local)
        .env("KANBAN_CONFIG_HOME", &config_home)
        .env("KANBAN_REPO_ROOT", &environment_backlog)
        .args(["config", "get", "theme.color_mode"])
        .output()
        .expect("kanban binary should run");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "always");
}

#[test]
fn invalid_configured_default_does_not_fall_back_to_current_directory() {
    let workspace = tempdir().expect("workspace should be created");
    let config_home = workspace.path().join("config");
    let service = workspace.path().join("service");
    std::fs::create_dir_all(&service).unwrap();
    git_init(&service);
    let config_dir = config_home.join("kanban");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.json"),
        r#"{"default_repo_root":"/does/not/exist"}"#,
    )
    .unwrap();

    let output = kanban_with_config_home(&service, &config_home, &["config", "show"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Configured kanban repository"));
}

#[test]
fn init_ignores_configured_default_and_initializes_the_current_repository() {
    let workspace = tempdir().expect("workspace should be created");
    let config_home = workspace.path().join("config");
    let backlog = workspace.path().join("backlog");
    let new_repository = workspace.path().join("new-repository");
    std::fs::create_dir_all(&backlog).unwrap();
    std::fs::create_dir_all(&new_repository).unwrap();
    init_kanban_repo(&backlog, &config_home);
    git_init(&new_repository);

    let set = kanban_with_config_home(
        &backlog,
        &config_home,
        &["config", "global", "set-root", backlog.to_str().unwrap()],
    );
    assert!(set.status.success());

    let init = kanban_with_config_home(&new_repository, &config_home, &["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(new_repository.join(".kanban/settings.json").is_file());
}
