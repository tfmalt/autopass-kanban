mod cli;
mod completion;
mod dispatch;
mod doctor_cli;
mod json_out;
mod layout;
mod ops;
mod outcome;
mod prompt;
mod render;
mod self_manage;
mod theme;
mod web;
mod web_process;

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use crate::cli::{Args, Command, OutputFormat, command_repo_root, theme_for_command};
use crate::dispatch::{DispatchMode, execute_command};
use crate::json_out::{emit_json_git_requirement_error, render_json};
use crate::outcome::print_human_outcome;
use crate::self_manage::latest_version_if_newer;
use crate::theme::Theme;
use kanban_core::{ColorMode, DEFAULT_REPO_ROOT_MARKER, effective_repo_root};

pub(crate) fn render_styled_output(styled: clap::builder::StyledStr, color: bool) -> String {
    if color {
        styled.ansi().to_string()
    } else {
        styled.to_string()
    }
}

pub(crate) fn render_version_line(theme: &Theme) -> String {
    format!(
        "{} {}",
        theme.brand(),
        theme.version(env!("CARGO_PKG_VERSION"))
    )
}

pub(crate) fn render_version_update_notice(theme: &Theme, latest: &str) -> String {
    format!(
        "{} A new version is available: {}. Run {} to update.",
        theme.info_label(),
        theme.version(latest),
        theme.command("kanban upgrade")
    )
}

pub(crate) fn render_no_args_help_output(theme: &Theme) -> Result<String> {
    let mut command = Args::command();
    let help = render_styled_output(command.render_help(), theme.color);
    Ok(format!("{}\n{help}\n", render_version_line(theme)))
}

pub(crate) fn command_requires_git_repository(command: &Command) -> bool {
    !matches!(
        command,
        Command::Upgrade { .. }
            | Command::Uninstall { .. }
            | Command::Completion { .. }
            | Command::Config {
                command: crate::cli::ConfigCommand::Global { .. }
            }
    )
}

pub(crate) fn command_git_requirement_path(command: &Command) -> &Path {
    command_repo_root(command)
        .map(PathBuf::as_path)
        .unwrap_or_else(|| Path::new("."))
}

fn effective_command_git_requirement_path(command: &Command) -> Result<PathBuf> {
    let path = command_git_requirement_path(command);
    if path == Path::new(DEFAULT_REPO_ROOT_MARKER) {
        effective_repo_root()
    } else {
        Ok(path.to_path_buf())
    }
}

pub(crate) fn git_repository_requirement_message(path: &Path) -> String {
    if path == Path::new(".") {
        "Current directory is not a git repository. Run `git init` to initialize it.".into()
    } else {
        format!(
            "Repository path {} is not a git repository. Run `git init` to initialize it.",
            path.display()
        )
    }
}

pub(crate) fn is_git_repository(path: &Path) -> bool {
    ProcessCommand::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(crate) fn command_git_requirement_error(command: &Command) -> Option<String> {
    if !command_requires_git_repository(command) {
        return None;
    }

    let path = match effective_command_git_requirement_path(command) {
        Ok(path) => path,
        Err(error) => return Some(error.to_string()),
    };
    if is_git_repository(&path) {
        None
    } else {
        Some(git_repository_requirement_message(&path))
    }
}

pub(crate) fn normalize_args(raw_args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    let doctor_passthrough = |arg: &std::ffi::OsString| {
        matches!(
            arg.to_str(),
            Some("show" | "fix" | "help" | "-h" | "--help")
        )
    };

    if raw_args.len() >= 2
        && raw_args.get(1).is_some_and(|arg| arg == "doctor")
        && raw_args.get(2).is_none_or(|arg| !doctor_passthrough(arg))
    {
        let mut normalized = Vec::with_capacity(raw_args.len() + 1);
        normalized.push(raw_args[0].clone());
        normalized.push(raw_args[1].clone());
        normalized.push(std::ffi::OsString::from("show"));
        normalized.extend(raw_args.into_iter().skip(2));
        normalized
    } else {
        raw_args
    }
}

fn main() {
    if let Err(error) = run() {
        let theme = Theme::for_stdout(ColorMode::Auto);
        eprintln!(
            "{} {}",
            theme.error_label(),
            theme.highlight_commands(&error.to_string())
        );
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let raw_args = normalize_args(std::env::args_os().collect::<Vec<_>>());
    if raw_args.len() == 2 && matches!(raw_args[1].to_str(), Some("--version" | "-V")) {
        let theme = Theme::for_stdout(ColorMode::Auto);
        println!("{}", render_version_line(&theme));
        if let Some(latest) = latest_version_if_newer() {
            println!("\n{}", render_version_update_notice(&theme, &latest));
        }
        return Ok(());
    }

    if raw_args.len() == 1 {
        let color_mode = kanban_core::load_kanban_config(".")
            .ok()
            .map(|config| config.theme.color_mode)
            .unwrap_or(ColorMode::Auto);
        let theme = Theme::for_stdout(color_mode);
        print!("{}", render_no_args_help_output(&theme)?);
        return Ok(());
    }

    let args = Args::parse_from(raw_args);

    if args.format == OutputFormat::Json {
        if let Some(message) = command_git_requirement_error(&args.command) {
            std::process::exit(emit_json_git_requirement_error(&args.command, message));
        }
        std::process::exit(render_json(&args.command));
    }

    if let Some(message) = command_git_requirement_error(&args.command) {
        bail!(message);
    }

    let theme = theme_for_command(&args.command);
    print_human_outcome(
        &theme,
        execute_command(&args.command, DispatchMode::Human, &theme)?,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{DoctorCommand, ROOT_HELP_GIT_REQUIREMENT};

    #[test]
    fn no_args_output_starts_with_version_line() {
        let output =
            render_no_args_help_output(&Theme::plain()).expect("no-args output should render");
        let first_line = output.lines().next().expect("output should have lines");

        assert_eq!(
            first_line,
            Args::command().render_version().to_string().trim_end()
        );
        assert!(output.contains("Usage: kanban"));
        assert!(output.contains(ROOT_HELP_GIT_REQUIREMENT));
    }

    #[test]
    fn no_args_output_can_emit_ansi_when_color_enabled() {
        let output =
            render_no_args_help_output(&Theme::color()).expect("no-args output should render");

        assert!(
            output.contains("\u{1b}["),
            "expected ansi color codes in help output"
        );
        assert!(output.contains("Commands:"));
    }

    #[test]
    fn version_update_notice_uses_info_label() {
        let output = render_version_update_notice(&Theme::plain(), "26.6.2501");

        assert_eq!(
            output,
            "ℹ info A new version is available: 26.6.2501. Run kanban upgrade to update."
        );
    }

    #[test]
    fn upgrade_is_exempt_from_git_repository_requirement() {
        let command = Command::Upgrade {
            prefix: None,
            skills_dir: None,
            no_skills: false,
            yes: false,
            force: false,
            dry_run: true,
            quiet: false,
        };

        assert!(!command_requires_git_repository(&command));
        assert!(command_git_requirement_error(&command).is_none());
    }

    #[test]
    fn init_defaults_to_current_directory_git_requirement_message() {
        let command = Command::Init {
            repo_root: PathBuf::from("."),
            no_sprints: false,
            no_epics: false,
            no_phases: false,
        };

        assert!(command_requires_git_repository(&command));
        assert_eq!(command_git_requirement_path(&command), Path::new("."));
        assert_eq!(
            git_repository_requirement_message(Path::new(".")),
            "Current directory is not a git repository. Run `git init` to initialize it."
        );
    }

    #[test]
    fn no_args_help_wraps_command_descriptions_into_two_columns() {
        let mut command = Args::command();
        command = command.term_width(60);
        let output = command.render_help().to_string();

        assert!(
            output.contains("  init        Initialize `.kanban` in the repository root."),
            "expected command and description to share the first help row"
        );
        assert!(
            output.contains("              Effect: creates default JSON config files in"),
            "expected wrapped continuation line to stay in the description column"
        );
        assert!(
            output.contains("              `.kanban/`. Side effects: no backlog files are"),
            "expected later wrapped lines to remain aligned"
        );
        assert!(
            output.contains("              modified."),
            "expected final wrapped line to remain aligned"
        );
    }

    #[test]
    fn bare_doctor_is_normalized_to_show() {
        let raw = normalize_args(vec!["kanban".into(), "doctor".into(), "/tmp/repo".into()]);
        let args = Args::parse_from(raw);

        match args.command {
            Command::Doctor {
                command: DoctorCommand::Show { repo_root },
            } => assert_eq!(repo_root, PathBuf::from("/tmp/repo")),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn doctor_help_is_not_normalized_to_show() {
        let raw = normalize_args(vec!["kanban".into(), "doctor".into(), "help".into()]);

        assert_eq!(raw, vec!["kanban", "doctor", "help"]);
    }

    #[test]
    fn doctor_flag_help_is_not_normalized_to_show() {
        let raw = normalize_args(vec!["kanban".into(), "doctor".into(), "--help".into()]);

        assert_eq!(raw, vec!["kanban", "doctor", "--help"]);
    }
}
