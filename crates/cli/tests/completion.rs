use std::process::{Command, Output};

use tempfile::tempdir;

fn kanban(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .args(args)
        .output()
        .expect("kanban binary should run")
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
    assert!(stdout.contains("web"));
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
    assert!(stdout.contains("web"));
}

#[test]
fn zsh_completion_includes_web_subcommands_and_flags() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("_kanban__subcmd__web_commands"));
    assert!(stdout.contains("'start:Start the local kanban web UI."));
    assert!(stdout.contains("'stop:Stop the local kanban web UI."));
    assert!(stdout.contains("'restart:Restart the local kanban web UI."));
    assert!(stdout.contains("'status:Show local kanban web UI process status."));
    assert!(stdout.contains("'log:Print the local kanban web UI log."));
    assert!(
        stdout.contains("'--foreground[Run in the foreground instead of writing a PID file.]'")
    );
    assert!(
        stdout
            .contains("'--open[Open the configured web URL in the default browser after start.]'")
    );
    assert!(stdout.contains("'--dev[Run the web server through npm run dev\\:server.]'"));
    assert!(
        stdout.contains("'--build[Build tools/kanban-web before starting in production mode.]'")
    );
}

#[test]
fn zsh_completion_does_not_treat_web_log_lines_as_files() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("'--lines=[Only print the last N log lines.]:N:'"));
    assert!(
        !stdout.contains("'--lines=[Only print the last N log lines.]:N:_default'"),
        "web log --lines should not fall back to file completion"
    );
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
fn zsh_completion_includes_dynamic_doctor_fix_target_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("_kanban_doctor_fix_targets"),
        "zsh completion should define _kanban_doctor_fix_targets"
    );
    assert!(
        stdout.contains("current -- current active sprint"),
        "doctor fix helper should include the literal current target"
    );
    assert!(
        stdout.contains("list-ids stories-with-titles"),
        "doctor fix helper should call `kanban list-ids stories-with-titles`"
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
fn zsh_completion_replaces_default_for_config_key_and_value_args() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("_kanban_config_keys"));
    assert!(stdout.contains("_kanban_config_values"));
    assert!(stdout.contains(
        "':key -- Configuration key, for example paths.backlog or theme.color_mode.:_kanban_config_keys'"
    ));
    assert!(stdout.contains(
        "':value -- Configuration value. Use comma-separated values for story_points.allowed_values.:_kanban_config_values'"
    ));
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
fn zsh_completion_does_not_treat_sprint_create_option_values_as_files() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("'--number=[Sprint number. Defaults to the next suggested number.]:N:'")
    );
    assert!(
        stdout.contains(
            "'--headline=[Sprint headline slug. Required in non-interactive mode.]:SLUG:'"
        )
    );
    assert!(stdout.contains(
        "'--start=[Start date. Defaults to the suggested next start date.]:YYYY-MM-DD:'"
    ));
    assert!(
        stdout.contains("'--end=[End date. Defaults to the suggested next end date.]:YYYY-MM-DD:'")
    );
    assert!(
        !stdout.contains(
            "'--number=[Sprint number. Defaults to the next suggested number.]:N:_default'"
        ),
        "sprint create --number should not fall back to file completion"
    );
    assert!(
        !stdout.contains(
            "'--headline=[Sprint headline slug. Required in non-interactive mode.]:SLUG:_default'"
        ),
        "sprint create --headline should not fall back to file completion"
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
fn zsh_completion_replaces_default_for_doctor_fix_target_arg() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(
            "Optional scope\\: a story id like US-F1-053 or the literal `current`.:_kanban_doctor_fix_targets"
        ),
        "doctor fix target arg should use _kanban_doctor_fix_targets"
    );
}

#[test]
fn zsh_completion_supports_bare_doctor_repo_root_completion() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("_kanban_doctor_command_or_repo_root"));
    assert!(stdout.contains("repo-root:repository root:_files -/"));
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
fn bash_completion_does_not_treat_sprint_create_option_values_as_files() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__sprint__subcmd__create"));
    assert!(stdout.contains(
        "opts=\"-h --number --headline --start --end --non-interactive --format --help [REPO_ROOT]\""
    ));
    assert!(stdout.contains("COMPREPLY=( $(compgen -W \"YYYY-MM-DD\" -- \"${cur}\") )"));
    assert!(
        !stdout.contains("kanban__subcmd__sprint__subcmd__create)\n            opts=\"-h --number --headline --start --end --non-interactive --help [REPO_ROOT]\"\n            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then\n                COMPREPLY=( $(compgen -W \"${opts}\" -- \"${cur}\") )\n                return 0\n            fi\n            case \"${prev}\" in\n                --number)\n                    COMPREPLY=($(compgen -f \"${cur}\"))"),
        "sprint create options should not use file completion"
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
fn bash_completion_includes_dynamic_doctor_fix_target_completion() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("kanban__subcmd__doctor__subcmd__fix"),
        "bash completion should include the doctor fix case block"
    );
    assert!(
        stdout.contains("current $(kanban list-ids stories 2>/dev/null)"),
        "bash completion should include current plus story IDs for doctor fix"
    );
}

#[test]
fn bash_completion_supports_bare_doctor_repo_root_completion() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__doctor)"));
    assert!(stdout.contains("doctor_commands=\"show fix help\""));
    assert!(stdout.contains("compgen -d -- \"${cur}\""));
}

#[test]
fn bash_completion_includes_dynamic_config_completion() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__config__subcmd__get"));
    assert!(stdout.contains("config_keys=\"paths.backlog paths.sprints theme.color_mode"));
    assert!(stdout.contains("color_modes=\"auto always never\""));
}

#[test]
fn bash_completion_includes_web_subcommands_and_flags() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__web)"));
    assert!(stdout.contains("opts=\"-h --format --help start stop restart status log help\""));
    assert!(stdout.contains("kanban__subcmd__web__subcmd__start)"));
    assert!(
        stdout
            .contains("opts=\"-h --foreground --open --dev --build --format --help [REPO_ROOT]\"")
    );
    assert!(stdout.contains("kanban__subcmd__web__subcmd__restart)"));
    assert!(stdout.contains("opts=\"-h --open --dev --build --format --help [REPO_ROOT]\""));
    assert!(stdout.contains("kanban__subcmd__web__subcmd__log)"));
    assert!(stdout.contains("opts=\"-f -h --lines --follow --format --help [REPO_ROOT]\""));
}

#[test]
fn bash_completion_does_not_treat_web_log_lines_as_files() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__web__subcmd__log"));
    assert!(stdout.contains("--lines)\n                    COMPREPLY=()"));
    assert!(
        !stdout.contains("--lines)\n                    COMPREPLY=($(compgen -f \"${cur}\"))"),
        "web log --lines should not use file completion"
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
fn sprint_create_help_explains_non_interactive_flags() {
    let output = kanban(&["sprint", "create", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("at least one of"));
    assert!(stdout.contains("--number/--headline/--start/--end is supplied"));
    assert!(stdout.contains("Non-interactive behavior:"));
    assert!(stdout.contains("`--headline` is required whenever flags are used"));
    assert!(stdout.contains("kanban sprint create --non-interactive --headline foundation"));
}

#[test]
fn doctor_help_subcommand_prints_doctor_help() {
    let output = kanban(&["doctor", "help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Usage: kanban doctor [REPO_ROOT]"));
    assert!(stdout.contains("kanban doctor help"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("fix"));
}

#[test]
fn doctor_flag_help_prints_doctor_help() {
    let output = kanban(&["doctor", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Usage: kanban doctor [REPO_ROOT]"));
    assert!(stdout.contains("kanban doctor help"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("fix"));
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
    let temp_root = tempdir().expect("temp repo should be created");
    let backlog_dir = temp_root
        .path()
        .join("delivery/backlog/phase-1-test/01.demo");
    std::fs::create_dir_all(&backlog_dir).expect("backlog dir should exist");
    std::fs::write(
        backlog_dir.join("US-F1-010-ci-pipeline-build-and-unit-tests.md"),
        "---\nid: US-F1-010\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: \nassignee: TBD\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story: CI pipeline with build and unit tests\n",
    )
    .expect("story fixture should be written");

    let repo_root = temp_root.path().display().to_string();
    let init_output = kanban(&["init", &repo_root]);
    assert!(init_output.status.success());

    let output = kanban(&["list-ids", "stories-with-titles", &repo_root]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("US-F1-010\tCI pipeline with build and unit tests"),
        "story completion listing should emit tab-separated ID and title"
    );
}

#[test]
fn bare_kanban_with_missing_config_prints_only_init_guidance() {
    let temp_root = tempdir().expect("temp repo should be created");
    let output = Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(temp_root.path())
        .output()
        .expect("kanban binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stdout.starts_with(&format!("kanban {}", env!("CARGO_PKG_VERSION"))),
        "stdout should start with the version line, got: {stdout}"
    );
    assert_eq!(
        stdout.trim_end().lines().count(),
        1,
        "stdout should only contain the version line, got: {stdout}"
    );
    assert!(
        stderr.starts_with("   "),
        "stderr should start with warning symbol, got: {stderr}"
    );
    assert!(stderr.contains("No `.kanban` configuration found"));
    assert!(stderr.contains("\n    Run `kanban init` to initialize this repository"));
}

#[test]
fn bare_kanban_with_config_prints_version_before_help() {
    let temp_root = tempdir().expect("temp repo should be created");
    let repo_root = temp_root.path().display().to_string();

    let init_output = kanban(&["init", &repo_root]);
    assert!(init_output.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(temp_root.path())
        .output()
        .expect("kanban binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");

    assert!(stderr.is_empty(), "stderr should be empty, got: {stderr}");
    assert!(
        stdout.starts_with(&format!("kanban {}\n", env!("CARGO_PKG_VERSION"))),
        "stdout should start with version on the first line, got: {stdout}"
    );
    assert!(stdout.contains("Markdown-first kanban tooling"));
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("kanban [OPTIONS] <COMMAND>") || stdout.contains("kanban <COMMAND>"));
}

#[test]
fn sprint_commands_use_theme_config_from_target_repo_root() {
    let temp_root = tempdir().expect("temp repo should be created");
    let repo_root = temp_root.path().display().to_string();

    let init_output = kanban(&["init", &repo_root]);
    assert!(init_output.status.success());

    let set_output = kanban(&["config", "set", "theme.color_mode", "always", &repo_root]);
    assert!(set_output.status.success());

    let backlog_root = temp_root.path().join("delivery/backlog");
    std::fs::create_dir_all(&backlog_root).expect("backlog root should exist");
    let sprint_root = temp_root.path().join("delivery/sprints");
    std::fs::create_dir_all(&sprint_root).expect("sprint dir should exist");
    std::fs::write(
        sprint_root.join("S001.foundation.md"),
        "---\nsprint: S001\nheadline: foundation\nstart_date: 2099-06-01\nend_date: 2099-06-12\nstatus: planned\nwip_limit: null\n---\n\n# S001: foundation\n",
    )
    .expect("sprint file should be written");

    let outside_root = tempdir().expect("outside dir should be created");
    let output = Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(outside_root.path())
        .args(["sprint", "list", &repo_root])
        .output()
        .expect("kanban binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("\u{1b}["),
        "expected ANSI styling from target repo config, got: {stdout}"
    );
}
