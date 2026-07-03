//! Regression tests for the human (non-JSON) CLI output path.
//!
//! These guard against behavior drift introduced by the single-dispatch
//! refactor in `crates/cli/src/dispatch.rs`. In particular, the human path
//! must keep emitting the pre-refactor feature-gate error message, including
//! the `(repo: ...)` suffix that the JSON path deliberately omits.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;

fn kanban_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("kanban binary should run")
}

fn git_init(dir: &Path) {
    let init = Command::new("git")
        .current_dir(dir)
        .args(["init"])
        .output()
        .expect("git init should run");
    assert!(
        init.status.success(),
        "git init should succeed; stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );
}

fn init_repo(dir: &Path) {
    git_init(dir);
    let repo_root = dir.to_string_lossy().into_owned();
    let output = kanban_in(dir, &["init", &repo_root]);
    assert!(
        output.status.success(),
        "kanban init should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sprint_list_human_reports_disabled_feature_with_repo_suffix() {
    let dir = tempdir().expect("temp dir should be created");
    let repo_root = dir.path().to_string_lossy().into_owned();
    init_repo(dir.path());
    fs::create_dir_all(dir.path().join("delivery/backlog")).expect("create backlog dir");
    fs::create_dir_all(dir.path().join("delivery/sprints")).expect("create sprints dir");

    let disable_output = kanban_in(dir.path(), &["features", "disable", "sprints", &repo_root]);
    assert!(
        disable_output.status.success(),
        "features disable sprints should succeed; stderr: {}",
        String::from_utf8_lossy(&disable_output.stderr)
    );

    // Run `kanban sprint list` WITHOUT --format json (human path).
    let output = kanban_in(dir.path(), &["sprint", "list", &repo_root]);

    assert!(
        !output.status.success(),
        "sprint list should fail when sprints feature is disabled"
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    let expected = format!(
        "\u{2716} error Feature 'sprints' is disabled in .kanban/settings.json. Run kanban features enable sprints to re-enable it. (repo: {repo_root})\n"
    );
    assert_eq!(
        stderr, expected,
        "human error message should match the pre-refactor message including the repo suffix"
    );
}
