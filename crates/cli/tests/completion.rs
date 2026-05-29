use std::process::{Command, Output};

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
