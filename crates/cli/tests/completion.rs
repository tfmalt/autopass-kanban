use std::process::{Command, Output};

use tempfile::tempdir;

fn kanban(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
        .args(args)
        .output()
        .expect("kanban binary should run")
}

fn git_init(dir: &std::path::Path) {
    let output = Command::new("git")
        .current_dir(dir)
        .args(["init"])
        .output()
        .expect("git init should run");
    assert!(
        output.status.success(),
        "git init should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
fn powershell_completion_covers_current_command_tree() {
    let output = kanban(&["completion", "powershell"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("Register-ArgumentCompleter"));
    assert!(stdout.contains("kanban"));
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
    assert!(stdout.contains(
        r"'--dev[Run the Vite frontend development server from \`web/\`. Use a separate \`kanban web serve\` process for live API requests.]'"
    ));
    assert!(stdout.contains(r"'--build[Build \`web/\` before starting in production mode.]'"));
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
        stdout.contains("compadd -U -d descriptions -a ids"),
        "zsh story helper should insert substring-filtered IDs while displaying descriptions"
    );
}

#[test]
fn zsh_story_completion_accepts_numeric_id_fragments() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(r#"local needle="$PREFIX""#)
            && stdout.contains(r#""${(L)id}" == *"${(L)needle}"*"#)
            && stdout.contains("compadd -U -d descriptions -a ids"),
        "zsh story completion should filter by case-insensitive substring and force-add non-prefix matches like 011 -> US-F*-011"
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
        stdout.contains("local -a matches=( current )")
            && stdout.contains("kanban list-ids stories 2>/dev/null")
            && stdout.contains("$id"),
        "bash completion should seed current and append story IDs for doctor fix"
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
    assert!(
        stdout.contains(
            "config_keys=\"paths.backlog paths.sprints features.sprints features.epics features.phases theme.color_mode"
        )
    );
    assert!(stdout.contains("color_modes=\"auto always never\""));
    assert!(stdout.contains("feature_flags=\"true false on off yes no 1 0\""));
}

#[test]
fn bash_completion_includes_web_subcommands_and_flags() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__web)"));
    assert!(
        stdout.contains("opts=\"-h --format --help serve start stop restart status log help\"")
    );
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
fn completion_help_explains_bash_zsh_and_powershell_setup() {
    for args in [["completion", "help"], ["completion", "--help"]] {
        let output = kanban(&args);

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("Install bash completion"));
        assert!(stdout.contains("kanban completion bash"));
        assert!(stdout.contains("Install zsh completion"));
        assert!(stdout.contains("kanban completion zsh"));
        assert!(stdout.contains("Install PowerShell completion"));
        assert!(stdout.contains("$PROFILE"));
        assert!(stdout.contains("kanban completion powershell"));
        assert!(stdout.contains("Supported shells: bash, zsh, powershell"));
    }
}

#[test]
fn sprint_create_help_explains_non_interactive_flags() {
    let output = kanban(&["sprint", "create", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let normalized_stdout = stdout.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(normalized_stdout.contains("at least one of"));
    assert!(normalized_stdout.contains("--number/--headline/--start/--end is supplied"));
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
    assert!(stderr.contains("powershell"));
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
    git_init(temp_root.path());
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
fn hidden_task_completion_listing_includes_task_ids_for_story() {
    let temp_root = tempdir().expect("temp repo should be created");
    let backlog_dir = temp_root
        .path()
        .join("delivery/backlog/phase-1-test/01.demo");
    std::fs::create_dir_all(&backlog_dir).expect("backlog dir should exist");
    std::fs::write(
        backlog_dir.join("US-F1-010-ci-pipeline-build-and-unit-tests.md"),
        "---\nid: US-F1-010\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n---\n# User Story: CI pipeline with build and unit tests\n",
    )
    .expect("story fixture should be written");
    std::fs::write(
        backlog_dir.join("US-F1-010-ci-pipeline-build-and-unit-tests.tasks.md"),
        "# Tasks for US-F1-010\n\nParent User Story: US-F1-010\nSprint: S001.foundation\n\n## TASK-US-F1-010-001 - First task\n\nStatus: todo\nTags: cli\n\nDescription:\nFirst.\n",
    )
    .expect("task fixture should be written");

    let repo_root = temp_root.path().display().to_string();
    git_init(temp_root.path());
    let init_output = kanban(&["init", &repo_root]);
    assert!(init_output.status.success());

    let output = kanban(&["list-task-ids", "US-F1-010", &repo_root]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("TASK-US-F1-010-001"));
}

#[test]
fn bare_kanban_with_missing_config_prints_help_and_git_requirement() {
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
    assert!(stdout.contains("Usage: kanban"));
    assert!(stdout.contains("Git requirement:"));
    assert!(stderr.is_empty(), "stderr should be empty, got: {stderr}");
}

#[test]
fn bare_kanban_with_config_prints_version_before_help() {
    let temp_root = tempdir().expect("temp repo should be created");
    let repo_root = temp_root.path().display().to_string();

    git_init(temp_root.path());
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
    assert!(stdout.contains("Git requirement:"));
}

#[test]
fn sprint_commands_use_theme_config_from_target_repo_root() {
    let temp_root = tempdir().expect("temp repo should be created");
    let repo_root = temp_root.path().display().to_string();

    git_init(temp_root.path());
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

#[test]
fn zsh_completion_includes_task_status_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("_kanban_task_statuses"),
        "zsh completion should define _kanban_task_statuses"
    );
    assert!(stdout.contains("todo"));
    assert!(stdout.contains("in-progress"));
    assert!(stdout.contains("blocked"));
}

#[test]
fn zsh_completion_includes_story_status_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("_kanban_story_statuses"),
        "zsh completion should define _kanban_story_statuses"
    );
    assert!(stdout.contains("backlog"));
    assert!(stdout.contains("todo"));
    assert!(stdout.contains("ready-for-qa"));
}

#[test]
fn zsh_completion_replaces_story_move_status_with_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("':status -- Target status, for example backlog, ready, todo, in-progress, ready-for-qa, done, or blocked.:_kanban_story_statuses'"),
        "story move status arg should use _kanban_story_statuses"
    );
}

#[test]
fn zsh_completion_replaces_remaining_domain_arguments() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains(
        "':phase -- Phase identifier to inspect, for example 1 or F1.:_kanban_phase_ids'"
    ));
    assert!(stdout.contains("'--sprint=[List stories assigned to the specified sprint, for example S001.foundation.]:ID:_kanban_sprint_names'"));
    assert!(stdout.contains("':id -- Story id to move, for example US-F1-053.:_kanban_story_ids'"));
    assert!(
        stdout.contains(
            "':id -- Backlog story id to plan, for example US-F2-001.:_kanban_story_ids'"
        )
    );
    assert!(stdout.contains("'--sprint=[Target sprint name or Snnn prefix, for example S001.planning or S001.]:SPRINT:_kanban_sprint_names'"));
    assert!(stdout.contains("':task_id -- Task id to update, for example TASK-US-F1-053-001.:_kanban_task_ids_for_story'"));
    assert!(stdout.contains("':task_id -- Task id to delete, for example TASK-US-F1-053-001.:_kanban_task_ids_for_story'"));
    assert!(stdout.contains("':story_id -- Story id whose task IDs should be listed, for example US-F1-053.:_kanban_story_ids'"));
}

#[test]
fn zsh_completion_does_not_use_default_for_free_text_values() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("'--title=[Task title to append to the sibling task log.]:TITLE:'"));
    assert!(
        stdout
            .contains("'--description=[Task description to write in the task log.]:DESCRIPTION:'")
    );
    assert!(stdout.contains("'--work-started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]::TIMESTAMP:'"));
    assert!(!stdout.contains("Story id to move, for example US-F1-053.:_default"));
    assert!(!stdout.contains("Backlog story id to plan, for example US-F2-001.:_default"));
    assert!(!stdout.contains("Task id to update, for example TASK-US-F1-053-001.:_default"));
}

#[test]
fn zsh_completion_replaces_task_status_with_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("'--status=[Initial task status to write. Defaults to todo.]:STATUS:_kanban_task_statuses'"),
        "task add --status option should use _kanban_task_statuses"
    );
    assert!(
        stdout.contains("'--status=[Replacement task status. Omitted means keep the current status.]:STATUS:_kanban_task_statuses'"),
        "task update --status option should use _kanban_task_statuses"
    );
}

#[test]
fn zsh_completion_replaces_story_plan_sprint_with_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(":_kanban_sprint_names'") && stdout.contains("Target sprint"),
        "story plan sprint arg should use _kanban_sprint_names"
    );
}

#[test]
fn zsh_completion_replaces_story_update_sprint_with_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("--sprint=[Update frontmatter sprint")
            && stdout.contains(":_kanban_sprint_names'"),
        "story update --sprint option should use _kanban_sprint_names"
    );
}

#[test]
fn zsh_completion_replaces_story_update_id_with_helper() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains(
            "':id -- Story id to update, for example US-F1-053.:_kanban_story_or_epic_ids'"
        ),
        "story update positional id should use _kanban_story_or_epic_ids"
    );
}

#[test]
fn zsh_completion_replaces_story_update_id_epic_and_options() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("'--id=[Update frontmatter id. Omit VALUE to prompt with the current value.]::ID:_kanban_story_or_epic_ids'"),
        "story update id option should use _kanban_story_or_epic_ids"
    );
    assert!(
        stdout.contains("'--epic=[Update frontmatter epic. Omit VALUE to prompt with the current value.]::EPIC:_kanban_epic_ids'"),
        "story update epic option should use _kanban_epic_ids"
    );
    assert!(
        stdout.contains("'--status=[Update frontmatter status. Omit VALUE to prompt with the current value.]::STATUS:_kanban_story_update_statuses'"),
        "story update status option should use _kanban_story_update_statuses"
    );
    assert!(
        stdout.contains("'--story-points=[Update frontmatter story_points. Omit VALUE to prompt with the current value.]::POINTS:_kanban_story_point_values'"),
        "story update story_points option should use _kanban_story_point_values"
    );
    assert!(
        stdout.contains("== *\"${(L)needle}\"*"),
        "partial matching should use case-insensitive substring checks"
    );
}

#[test]
fn bash_completion_includes_story_plan_completion() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("kanban__subcmd__story__subcmd__plan"),
        "bash completion should have story plan case block"
    );
    assert!(
        stdout.contains("list-ids sprints"),
        "bash story plan --sprint should complete with sprints"
    );
}

#[test]
fn bash_completion_replaces_remaining_domain_arguments() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__phase__subcmd__show"));
    assert!(stdout.contains("phases=\"F1 F2 F3 F4 F5 1 2 3 4 5\""));
    assert!(stdout.contains("kanban__subcmd__story__subcmd__list"));
    assert!(stdout.contains("--sprint)\n                    COMPREPLY=( $(compgen -W \"$(kanban list-ids sprints 2>/dev/null)\" -- \"${cur}\") )"));
    assert!(stdout.contains(
        "story_statuses=\"draft backlog ready todo in-progress ready-for-qa blocked done dropped\""
    ));
    assert!(stdout.contains("resolved_story=$(_kanban_resolve_story_id \"${prev}\")"));
    assert!(stdout.contains("kanban list-task-ids \"${resolved_story}\" 2>/dev/null"));
    assert!(stdout.contains("kanban__subcmd__list__subcmd__task__subcmd__ids"));
    assert!(!stdout.contains("kanban__subcmd__story__subcmd__plan)\n            opts=\"-h --sprint --format --help <ID> [REPO_ROOT]\"\n            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]]"));
}

#[test]
fn zsh_task_completion_resolves_story_before_listing_task_ids() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("_kanban_resolve_story_id"));
    assert!(stdout.contains("story_id=$(_kanban_resolve_story_id \"${words[CURRENT-1]}\")"));
    assert!(stdout.contains("[[ -z \"$story_id\" ]] && return 0"));
}

#[test]
fn bash_task_completion_resolves_story_before_listing_task_ids() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("_kanban_resolve_story_id() {"));
    assert!(stdout.contains("resolved_story=$(_kanban_resolve_story_id \"${prev}\")"));
    assert!(stdout.contains("done < <(kanban list-task-ids \"${resolved_story}\" 2>/dev/null)"));
}

#[test]
fn bash_task_delete_completion_uses_story_scoped_task_ids() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kanban__subcmd__task__subcmd__delete)"));
    assert!(stdout.contains("opts=\"-h --format --help <STORY_ID> <TASK_ID> [REPO_ROOT]\""));
    assert!(stdout.contains("resolved_story=$(_kanban_resolve_story_id \"${prev}\")"));
}

#[test]
fn bash_story_completion_accepts_numeric_id_fragments() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("kanban__subcmd__story__subcmd__plan")
            && stdout.contains("while IFS= read -r id; do")
            && stdout.contains(r#"_kanban_ci_match "$id" "${cur}""#)
            && stdout.contains(r#"COMPREPLY=( "${matches[@]}" )"#),
        "bash story completion should use case-insensitive substring matching instead of compgen prefix matching"
    );
}

#[test]
fn bash_completion_replaces_story_update_id_with_helper() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("kanban__subcmd__story__subcmd__update)")
            && stdout.contains("local -a matches=()")
            && stdout.contains("kanban list-ids stories 2>/dev/null")
            && stdout.contains("kanban list-ids epics 2>/dev/null")
            && stdout.contains(r#"_kanban_ci_match "$id" "${cur}""#)
            && stdout.contains("COMPREPLY=( \"${matches[@]}\" )"),
        "story update positional id should complete with substring matching against stories and epics"
    );
}

#[test]
fn bash_completion_defines_case_insensitive_match_helper() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("_kanban_ci_match() {") && stdout.contains("tr '[:upper:]' '[:lower:]'"),
        "bash completion should define a case-insensitive substring match helper"
    );
}

#[test]
fn bash_completion_registers_kb_alias() {
    let output = kanban(&["completion", "bash"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("complete -F _kanban -o nosort -o bashdefault -o default kb")
            && stdout.contains("complete -F _kanban -o bashdefault -o default kb"),
        "bash completion should register the kb alias for both bash 4+ and older bash"
    );
}

#[test]
fn zsh_completion_registers_kb_alias() {
    let output = kanban(&["completion", "zsh"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("compdef _kanban kb"),
        "zsh completion should register the kb alias via compdef"
    );
}
#[test]
fn completion_status_lists_match_canonical_consts() {
    // US-018: verify the completion scripts emit exactly the canonical story
    // and task status lists, proving the single-source-of-truth invariant.
    let story_statuses = kanban_core::CANONICAL_STORY_STATUSES;
    let task_statuses = kanban_core::CANONICAL_TASK_STATUSES;

    let bash = kanban(&["completion", "bash"]);
    assert!(bash.status.success());
    let bash_stdout = String::from_utf8(bash.stdout).expect("bash stdout utf8");

    let zsh = kanban(&["completion", "zsh"]);
    assert!(zsh.status.success());
    let zsh_stdout = String::from_utf8(zsh.stdout).expect("zsh stdout utf8");

    for status in story_statuses {
        assert!(
            bash_stdout.contains(status),
            "bash completion must contain story status '{status}'"
        );
        assert!(
            zsh_stdout.contains(status),
            "zsh completion must contain story status '{status}'"
        );
    }
    for status in task_statuses {
        assert!(
            bash_stdout.contains(status),
            "bash completion must contain task status '{status}'"
        );
        assert!(
            zsh_stdout.contains(status),
            "zsh completion must contain task status '{status}'"
        );
    }

    // Verify no extra statuses leaked into the bash story_statuses variable.
    let story_statuses_line = bash_stdout
        .lines()
        .find(|line| line.contains("story_statuses="))
        .expect("bash completion must define story_statuses");
    let bash_story_values: Vec<&str> = story_statuses_line
        .split('"')
        .nth(1)
        .unwrap_or("")
        .split_whitespace()
        .collect();
    assert_eq!(
        bash_story_values, story_statuses,
        "bash story_statuses must exactly match CANONICAL_STORY_STATUSES"
    );
}

// ── US-019: Round-trip tests for completion enhancement ─────────────────────

/// Assert that a zsh helper is wired into a specific argument description,
/// not left as `_default`. If the `str::replace` that wires the helper fails
/// silently, the argument will still say `:_default` and this test will catch
/// the regression.
fn assert_zsh_helper_wired(stdout: &str, helper: &str, context: &str) {
    assert!(
        stdout.contains(helper),
        "zsh completion must define helper '{helper}' ({context})"
    );
    // The helper must be attached to at least one argument line inside the
    // `_arguments` block, not just defined as a standalone function. Argument
    // lines carry the helper name after a `:` separator and are quoted. We
    // check for the helper on a line that looks like an argument spec (starts
    // with whitespace + quote, contains the helper name).
    let wired = stdout.lines().any(|line| {
        line.contains(helper)
            && line.trim_start_matches(' ').starts_with('\'')
            && !line.contains("()")
    });
    assert!(
        wired,
        "zsh completion must wire '{helper}' into an argument ({context}); \
         it may have silently regressed to _default"
    );
}

#[test]
fn zsh_completion_wires_sprint_names_to_sprint_show() {
    let output = kanban(&["completion", "zsh"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("zsh stdout utf8");
    assert_zsh_helper_wired(&stdout, "_kanban_sprint_names", "sprint show argument");
}

#[test]
fn zsh_completion_wires_story_ids_to_story_commands() {
    let output = kanban(&["completion", "zsh"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("zsh stdout utf8");
    assert_zsh_helper_wired(
        &stdout,
        "_kanban_story_ids",
        "story show/move/delete argument",
    );
}

#[test]
fn zsh_completion_wires_status_helpers() {
    let output = kanban(&["completion", "zsh"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("zsh stdout utf8");
    assert_zsh_helper_wired(
        &stdout,
        "_kanban_story_statuses",
        "story move status argument",
    );
    assert_zsh_helper_wired(
        &stdout,
        "_kanban_story_update_statuses",
        "story update status argument",
    );
    assert_zsh_helper_wired(
        &stdout,
        "_kanban_task_statuses",
        "task add/update status argument",
    );
}

#[test]
fn zsh_completion_wires_epic_and_phase_and_config_helpers() {
    let output = kanban(&["completion", "zsh"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("zsh stdout utf8");
    assert_zsh_helper_wired(&stdout, "_kanban_epic_ids", "story update epic argument");
    assert_zsh_helper_wired(&stdout, "_kanban_phase_ids", "phase show argument");
    assert_zsh_helper_wired(&stdout, "_kanban_config_keys", "config get key argument");
}

/// Assert that a marker string unique to an `inject_bash_*` replacement is
/// present in the generated bash script. If the injection silently no-ops
/// (because the `old` pattern no longer matches the generated output), the
/// marker will be absent and this test will catch the regression.
fn assert_bash_marker_present(stdout: &str, marker: &str, injection_name: &str) {
    assert!(
        stdout.contains(marker),
        "bash completion must contain marker '{marker}' from {injection_name}; \
         the injection may have silently failed to apply"
    );
}

#[test]
fn bash_completion_story_move_status_injection_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // The story_statuses variable is only defined in the injected replacement.
    assert_bash_marker_present(&stdout, "story_statuses=", "inject_bash_story_move_status");
    // Dynamic story ID lookup is only in the replacement.
    assert_bash_marker_present(
        &stdout,
        "kanban list-ids stories",
        "inject_bash_story_move_status",
    );
}

#[test]
fn bash_completion_task_status_injections_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // task_statuses is defined in both task add and task update replacements.
    let count = stdout.matches("task_statuses=").count();
    assert!(
        count >= 2,
        "bash completion must have task_statuses= from both inject_bash_task_add_status \
         and inject_bash_task_update_status; found {count} occurrences"
    );
}

#[test]
fn bash_completion_doctor_fix_target_injection_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // `local -a matches=( current )` is only in the doctor fix target replacement.
    assert_bash_marker_present(
        &stdout,
        "matches=( current )",
        "inject_bash_doctor_fix_target",
    );
}

#[test]
fn bash_completion_sprint_create_injection_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // The sprint create replacement adds a dynamic sprint-number suggestion.
    assert_bash_marker_present(
        &stdout,
        "kanban list-ids sprints",
        "inject_bash_sprint_create",
    );
}

#[test]
fn bash_completion_story_plan_injection_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // The story plan replacement adds dynamic sprint-name and story-id lookup.
    assert_bash_marker_present(&stdout, "kanban list-ids stories", "inject_bash_story_plan");
}

#[test]
fn bash_completion_config_injections_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // Config get/set replacements add dynamic config-key lookup.
    assert_bash_marker_present(&stdout, "kanban config get", "inject_bash_config_get/set");
}

#[test]
fn bash_completion_story_update_injection_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // The story update replacement adds dynamic epic/sprint/status lookup.
    assert_bash_marker_present(&stdout, "kanban list-ids epics", "inject_bash_story_update");
}

#[test]
fn bash_completion_phase_show_injection_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // The phase show replacement adds a `phases=` variable with the phase
    // identifiers — this marker only exists in the injected replacement.
    assert_bash_marker_present(&stdout, "phases=\"F1", "inject_bash_phase_show");
}

#[test]
fn bash_completion_task_delete_injection_applied() {
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");
    // The task delete replacement adds dynamic task ID lookup.
    assert_bash_marker_present(&stdout, "kanban list-task-ids", "inject_bash_task_delete");
}

#[test]
fn bash_completion_no_remaining_default_case_blocks_for_enhanced_commands() {
    // US-019 scenario 3: if a `str::replace` fails silently, the original
    // case block stays with a bare `COMPREPLY=()` default. Verify that key
    // enhanced commands no longer have the unmodified default pattern.
    let output = kanban(&["completion", "bash"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("bash stdout utf8");

    // The story move case block must have been replaced — the original has
    // no `story_statuses` variable. If it's still the original, the marker
    // `story_statuses=` would be absent (already checked above), but we also
    // verify the replacement's dynamic lookup block is present.
    let story_move_block = stdout
        .find("kanban__subcmd__story__subcmd__move)")
        .and_then(|start| {
            stdout[start..]
                .find(";;")
                .map(|end| &stdout[start..start + end])
        })
        .expect("story move case block must exist");
    assert!(
        story_move_block.contains("kanban list-ids stories"),
        "story move case block must contain dynamic story ID lookup"
    );
}
