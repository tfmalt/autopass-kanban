use std::path::PathBuf;
use std::process::{Command, Output};

fn kanban(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .args(args)
        .output()
        .expect("kanban binary should run")
}

fn repo_root() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../")
        .canonicalize()
        .expect("repo root should resolve")
        .display()
        .to_string()
}

#[test]
fn bash_completion_covers_current_command_tree() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("_kanban()"));
    assert!(stdout.contains("complete"));
    assert!(stdout.contains("sprint"));
    assert!(stdout.contains("story"));
    assert!(stdout.contains("task"));
}

#[test]
fn zsh_completion_covers_current_command_tree() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("#compdef kanban"));
    assert!(stdout.contains("_kanban"));
    assert!(stdout.contains("sprint"));
    assert!(stdout.contains("story"));
    assert!(stdout.contains("task"));
}

#[test]
fn zsh_completion_includes_dynamic_sprint_name_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("_kanban_sprint_names"),
        "zsh completion should define _kanban_sprint_names"
    );
    assert!(
        stdout.contains("list-ids sprints"),
        "zsh sprint helper should call `kanban list-ids sprints`"
    );
}

#[test]
fn zsh_completion_includes_dynamic_story_id_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("_kanban_story_ids"),
        "zsh completion should define _kanban_story_ids"
    );
    assert!(
        stdout.contains("list-ids stories-with-titles"),
        "zsh story helper should call `kanban list-ids stories-with-titles`"
    );
    assert!(
        stdout.contains("compadd -d descriptions -a ids"),
        "zsh story helper should insert only IDs while displaying descriptions"
    );
}

#[test]
fn zsh_completion_helpers_are_redefined_when_completion_is_reevaluated() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        !stdout.contains("$+functions[_kanban_story_ids]"),
        "zsh helper definitions must not be guarded because that preserves stale helpers after re-eval"
    );
}

#[test]
fn zsh_completion_includes_dynamic_epic_id_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("_kanban_epic_ids"),
        "zsh completion should define _kanban_epic_ids"
    );
    assert!(
        stdout.contains("list-ids epics"),
        "zsh epic helper should call `kanban list-ids epics`"
    );
}

#[test]
fn zsh_completion_replaces_default_for_sprint_name_args() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    // sprint show and sprint rollover name args should use dynamic helper, not _default
    assert!(
        stdout.contains(":_kanban_sprint_names"),
        "sprint name args should use _kanban_sprint_names, not _default"
    );
    assert!(
        !stdout.contains("Sprint folder name to inspect, for example S001.foundation.:_default"),
        "sprint show name arg should not use _default"
    );
    assert!(
        !stdout.contains("Sprint folder name to close and roll over.:_default"),
        "sprint rollover name arg should not use _default"
    );
}

#[test]
fn zsh_completion_replaces_default_for_story_id_args() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(":_kanban_story_ids"),
        "story/task ID args should use _kanban_story_ids, not _default"
    );
    assert!(
        !stdout.contains("Story id to inspect, for example US-F1-053.:_default"),
        "story show id arg should not use _default"
    );
    assert!(
        !stdout.contains("Sprint story id to move, for example US-F1-053.:_default"),
        "story move id arg should not use _default"
    );
    assert!(
        !stdout.contains("Parent story id for the task, for example US-F1-053.:_default"),
        "task story_id arg should not use _default"
    );
}

#[test]
fn bash_completion_includes_dynamic_sprint_completion() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("list-ids sprints"),
        "bash completion should include `kanban list-ids sprints` for sprint show/rollover"
    );
}

#[test]
fn bash_completion_includes_dynamic_story_completion() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("list-ids stories"),
        "bash completion should include `kanban list-ids stories` for story/task commands"
    );
}

#[test]
fn completion_help_explains_bash_and_zsh_setup() {
    for args in [["completion", "help"], ["completion", "--help"]] {
        let output = kanban(&args);

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("Install bash completion"));
        assert!(stdout.contains("kanban completion bash"));
        assert!(stdout.contains("Install zsh completion"));
        assert!(stdout.contains("kanban completion zsh"));
        assert!(stdout.contains("Supported shells: bash, zsh"));
    }
}

#[test]
fn unsupported_completion_shell_fails_clearly() {
    let output = kanban(&["completion", "fish"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("invalid value 'fish'"));
    assert!(stderr.contains("bash"));
    assert!(stderr.contains("zsh"));
}

#[test]
fn hidden_story_completion_listing_includes_ids_and_titles() {
    let repo_root = repo_root();
    let output = kanban(&["list-ids", "stories-with-titles", &repo_root]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("US-F1-010\tCI pipeline with build and unit tests"),
        "story completion listing should emit tab-separated ID and title"
    );
}
