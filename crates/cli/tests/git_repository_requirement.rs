use std::process::{Command, Output};

use tempfile::tempdir;

fn kanban_in(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("kanban binary should run")
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
