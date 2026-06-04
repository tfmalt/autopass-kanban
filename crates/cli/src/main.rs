mod cli;
mod completion;
mod doctor_cli;
mod json_out;
mod layout;
mod prompt;
mod render;
mod theme;
mod web;

pub(crate) mod prelude {
    #[allow(unused_imports)]
    pub(crate) use anyhow::{Context, Result, bail};
    #[allow(unused_imports)]
    pub(crate) use chrono::NaiveDate;
    #[allow(unused_imports)]
    pub(crate) use clap::{CommandFactory, Parser, ValueEnum};
    #[allow(unused_imports)]
    pub(crate) use serde::Serialize;
    #[allow(unused_imports)]
    pub(crate) use std::collections::BTreeMap;
    #[allow(unused_imports)]
    pub(crate) use std::fs::{self, OpenOptions};
    #[allow(unused_imports)]
    pub(crate) use std::io::{ErrorKind, IsTerminal, Read, Seek, SeekFrom, Write};
    #[allow(unused_imports)]
    pub(crate) use std::net::TcpListener;
    #[allow(unused_imports)]
    pub(crate) use std::os::unix::process::CommandExt;
    #[allow(unused_imports)]
    pub(crate) use std::path::{Path, PathBuf};
    #[allow(unused_imports)]
    pub(crate) use std::process::{Command as ProcessCommand, Stdio};
    #[allow(unused_imports)]
    pub(crate) use std::thread;
    #[allow(unused_imports)]
    pub(crate) use std::time::Duration;
}

#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*,
    web::*,
};
use clap::{CommandFactory, Parser};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn render_styled_output(styled: clap::builder::StyledStr, color: bool) -> String {
    if color {
        styled.ansi().to_string()
    } else {
        styled.to_string()
    }
}

pub(crate) fn render_no_args_help_output(theme: &Theme) -> Result<String> {
    let version = Args::command().render_version().to_string();
    let mut command = Args::command();
    let help = render_styled_output(command.render_help(), theme.color);
    Ok(format!("{version}{help}\n"))
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

fn main() -> Result<()> {
    let raw_args = normalize_args(std::env::args_os().collect::<Vec<_>>());
    if raw_args.len() == 1 {
        let version_line = Args::command().render_version().to_string();

        let config = kanban_core::load_kanban_config(".");

        if let Err(error) = &config {
            println!("{}", version_line.trim_end());
            let theme = Theme::for_stdout(ColorMode::Auto);
            let message = error.to_string();
            let init_guidance = "Run `kanban init` to initialize this repository.";
            let primary = message
                .strip_suffix(&format!(" {init_guidance}"))
                .unwrap_or(message.as_str());
            eprintln!(" {}  {primary}", theme.warning(""));
            eprintln!("    {init_guidance}");
            return Ok(());
        }

        let theme = Theme::for_stdout(config?.theme.color_mode);
        print!("{}", render_no_args_help_output(&theme)?);
        return Ok(());
    }

    let args = Args::parse_from(raw_args);

    if args.format == OutputFormat::Json {
        std::process::exit(emit_json(&args.command));
    }

    let theme = theme_for_command(&args.command);

    match args.command {
        Command::Init { repo_root } => {
            let result = init_config(repo_root)?;
            println!(
                "{} {}",
                theme.success("Initialized config:"),
                theme.path(result.config_dir.display())
            );
            if result.created_files.is_empty() {
                println!("{} none", theme.label("Created files:"));
            } else {
                for file in result.created_files {
                    println!("- {}", theme.path(file.display()));
                }
            }
        }
        Command::Config { command } => match command {
            ConfigCommand::Show { repo_root } => {
                println!("{}", get_config_json(repo_root)?);
            }
            ConfigCommand::Get { key, repo_root } => {
                println!("{}", get_config_value(repo_root, &key)?);
            }
            ConfigCommand::Set {
                key,
                value,
                repo_root,
            } => {
                let result = set_config_value(repo_root, &key, &value)?;
                println!(
                    "{} {} = {}",
                    theme.success("Updated"),
                    theme.id(&result.key),
                    result.value
                );
                println!(
                    "{} {}",
                    theme.label("File:"),
                    theme.path(result.file_path.display())
                );
            }
        },
        Command::Sprint { command } => match command {
            SprintCommand::Current { repo_root } => {
                let sprint = summarize_current_sprint(repo_root)?;
                print_sprint_overview(&theme, OutputLayout::for_stdout()?, &sprint);
            }
            SprintCommand::List { repo_root } => {
                let sprints = summarize_sprints(repo_root)?;
                for sprint in sprints {
                    println!(
                        "- {} [{}..{}]{}",
                        theme.id(sprint.sprint_name),
                        sprint.start_date,
                        sprint.end_date,
                        sprint
                            .readme_status
                            .as_deref()
                            .map(|status| format!(" README={}", theme.status(status)))
                            .unwrap_or_default()
                    );
                }
            }
            SprintCommand::Show {
                name,
                short,
                repo_root,
            } => {
                let sprint = if let Some(name) = name {
                    summarize_sprint(repo_root, &name)?
                } else {
                    summarize_current_sprint(repo_root)?
                };
                if short {
                    print_sprint_overview_short(&theme, OutputLayout::for_stdout()?, &sprint);
                } else {
                    print_sprint_overview(&theme, OutputLayout::for_stdout()?, &sprint);
                }
            }
            SprintCommand::Create {
                number,
                headline,
                start,
                end,
                non_interactive,
                repo_root,
            } => {
                let any_flag =
                    number.is_some() || headline.is_some() || start.is_some() || end.is_some();
                let input = if non_interactive || any_flag {
                    let headline = headline.ok_or_else(|| {
                        anyhow::anyhow!(
                            "--headline is required when creating a sprint non-interactively."
                        )
                    })?;
                    let number = match number {
                        Some(value) => value,
                        None => suggested_sprint_defaults(&repo_root)?.0,
                    };
                    let repo_suggestion = suggested_sprint_defaults(&repo_root)?.1;
                    let today = chrono::Local::now().date_naive();
                    let start_date = match start {
                        Some(value) => NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
                            .map_err(|_| {
                                anyhow::anyhow!("--start must be a date as YYYY-MM-DD.")
                            })?,
                        None => repo_suggestion
                            .map(|(start_date, _)| start_date)
                            .unwrap_or(today),
                    };
                    let end_date = match end {
                        Some(value) => NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
                            .map_err(|_| anyhow::anyhow!("--end must be a date as YYYY-MM-DD."))?,
                        None => repo_suggestion
                            .map(|(_, end_date)| end_date)
                            .unwrap_or_else(|| suggested_sprint_dates(start_date).1),
                    };
                    CreateSprintInput {
                        number,
                        start_date,
                        end_date,
                        headline,
                    }
                } else {
                    prompt_create_sprint(&repo_root, None, None)?
                };
                let result = create_sprint(repo_root, &input)?;
                println!(
                    "{} {}",
                    theme.success("Created sprint:"),
                    result.sprint_name
                );
                println!(
                    "{} {}",
                    theme.label("Path:"),
                    theme.path(result.sprint_path.display())
                );
            }
            SprintCommand::Rollover { name, repo_root } => {
                let sprint = summarize_sprint(&repo_root, &name)?;
                let current_end = NaiveDate::parse_from_str(&sprint.end_date, "%Y-%m-%d")?;
                let (suggested_start, suggested_end) = suggested_sprint_dates(current_end);
                let next_input = if summarize_sprints(&repo_root)?.iter().any(|candidate| {
                    kanban_core::suggested_next_sprint_number(&repo_root)
                        .ok()
                        .map(|next_number| {
                            candidate
                                .sprint_name
                                .starts_with(&format!("S{next_number:03}."))
                        })
                        .unwrap_or(false)
                }) {
                    None
                } else {
                    Some(prompt_create_sprint(
                        &repo_root,
                        Some(suggested_start),
                        Some(suggested_end),
                    )?)
                };
                let result = rollover_sprint(&repo_root, &name, next_input.as_ref())?;
                print_rollover_result(&theme, &result);
            }
            SprintCommand::Sync { repo_root } => {
                let changed = sync_sprint_rosters(repo_root)?;
                if changed.is_empty() {
                    println!(
                        "{}",
                        theme.success("Sprint story tables already up to date.")
                    );
                } else {
                    println!("{}", theme.success("Regenerated sprint story tables:"));
                    for sprint in changed {
                        println!("- {}", theme.id(sprint));
                    }
                }
            }
        },
        Command::Phase { command } => match command {
            PhaseCommand::Show { phase, repo_root } => {
                let phase = summarize_phase(repo_root, &phase)?;
                print_phase_overview(&theme, OutputLayout::for_stdout()?, &phase);
            }
        },
        Command::Story { command } => match command {
            StoryCommand::Show { id, repo_root } => match find_story(repo_root, &id)? {
                Some(details) => print_story_details(&theme, OutputLayout::for_stdout()?, &details),
                None => println!("{} {id}", theme.warning("Story not found:")),
            },
            StoryCommand::List {
                current,
                all,
                next,
                sprint,
                repo_root,
            } => {
                let (scope, stories) = if all {
                    ("all stories".to_string(), list_all_stories(repo_root)?)
                } else if next {
                    let (sprint_name, stories) = list_next_sprint_stories(repo_root)?;
                    (format!("next sprint ({sprint_name})"), stories)
                } else if let Some(sprint_name) = sprint {
                    (
                        format!("sprint {sprint_name}"),
                        list_stories_in_sprint(repo_root, &sprint_name)?,
                    )
                } else {
                    let (sprint_name, stories) = list_current_sprint_stories(repo_root)?;
                    let label = if current {
                        format!("current sprint ({sprint_name})")
                    } else {
                        format!("active sprint ({sprint_name})")
                    };
                    (label, stories)
                };
                print_story_list(&theme, &scope, &stories);
            }
            StoryCommand::Move {
                id,
                status,
                assignee,
                repo_root,
            } => {
                let result = move_story_to_status_with_assignee(
                    repo_root,
                    &id,
                    &status,
                    assignee.as_deref(),
                )?;
                println!(
                    "{} {} in {}: {} -> {}",
                    theme.success("Moved"),
                    theme.id(&result.story_id),
                    result.sprint_name,
                    theme.status(&result.from_status),
                    theme.status(&result.to_status)
                );
                println!(
                    "{} {}",
                    theme.label("Story:"),
                    theme.path(result.story_path.display())
                );
                if let Some(task_path) = result.task_path {
                    println!(
                        "{} {}",
                        theme.label("Task file:"),
                        theme.path(task_path.display())
                    );
                }
            }
            StoryCommand::Plan {
                id,
                sprint,
                repo_root,
            } => {
                let result = plan_story_into_sprint(repo_root, &id, &sprint)?;
                println!(
                    "{} {} -> {}",
                    theme.success("Planned"),
                    theme.id(&result.story_id),
                    result.sprint_name
                );
                println!(
                    "{} {}",
                    theme.label("Story:"),
                    theme.path(result.story_path.display())
                );
                if let Some(task_path) = result.task_path {
                    println!(
                        "{} {}",
                        theme.label("Tasks:"),
                        theme.path(task_path.display())
                    );
                }
            }
            StoryCommand::Update {
                id,
                frontmatter_id,
                story_type,
                status,
                epic,
                sprint,
                story_points,
                assignee,
                activated,
                work_started,
                work_done,
                created,
                updated,
                task_file,
                repo_root,
            } => {
                let story_file = story_markdown_file(&repo_root, &id)?;
                let story = read_story_file(&story_file.absolute_path, &repo_root)?;
                let mut updates = Vec::new();
                for (field_name, option) in [
                    ("id", frontmatter_id),
                    ("type", story_type),
                    ("status", status),
                    ("epic", epic),
                    ("sprint", sprint),
                    ("story_points", story_points),
                    ("assignee", assignee),
                    ("activated", activated),
                    ("work_started", work_started),
                    ("work_done", work_done),
                    ("created", created),
                    ("updated", updated),
                    ("task_file", task_file),
                ] {
                    if let Some(update) =
                        story_frontmatter_update_value(&story, field_name, &option)?
                    {
                        updates.push(update);
                    }
                }

                if updates.is_empty() {
                    open_story_markdown_in_editor(&story_file.absolute_path)?;
                    println!(
                        "{} {}",
                        theme.success("Edited"),
                        theme.path(story_file.story_path.display())
                    );
                } else {
                    let result = update_story_frontmatter(&repo_root, &id, &updates)?;
                    println!(
                        "{} {} ({})",
                        theme.success("Updated"),
                        theme.id(&result.story_id),
                        result.updated_fields.join(", ")
                    );
                    println!(
                        "{} {}",
                        theme.label("Story:"),
                        theme.path(result.story_path.display())
                    );
                }
            }
        },
        Command::Task { command } => match command {
            TaskCommand::Add {
                story_id,
                title,
                status,
                tags,
                description,
                repo_root,
            } => {
                let result =
                    add_task_to_story(repo_root, &story_id, &title, &status, &tags, &description)?;
                println!(
                    "{} {} to {}",
                    theme.success("Added"),
                    theme.id(&result.task_id),
                    theme.id(&result.story_id)
                );
                println!(
                    "{} {}",
                    theme.label("Task file:"),
                    theme.path(result.task_file_path.display())
                );
            }
            TaskCommand::Update {
                story_id,
                task_id,
                title,
                status,
                tags,
                description,
                repo_root,
            } => {
                let result = update_task_in_story(
                    repo_root,
                    &story_id,
                    &task_id,
                    status.as_deref(),
                    title.as_deref(),
                    tags.as_deref(),
                    description.as_deref(),
                )?;
                println!(
                    "{} {} in {}",
                    theme.success("Updated"),
                    theme.id(&result.task_id),
                    theme.id(&result.story_id)
                );
                println!(
                    "{} {}",
                    theme.label("Task file:"),
                    theme.path(result.task_file_path.display())
                );
            }
        },
        Command::Web { command } => match command {
            WebCommand::Start {
                foreground,
                open,
                dev,
                build,
                repo_root,
            } => start_web(&theme, &repo_root, foreground, open, dev, build)?,
            WebCommand::Stop { repo_root } => {
                stop_web(&theme, &repo_root, false)?;
            }
            WebCommand::Restart {
                open,
                dev,
                build,
                repo_root,
            } => {
                stop_web(&theme, &repo_root, true)?;
                start_web(&theme, &repo_root, false, open, dev, build)?;
            }
            WebCommand::Status { repo_root } => print_web_status(&theme, &repo_root)?,
            WebCommand::Log {
                lines,
                follow,
                repo_root,
            } => print_web_log(&theme, &repo_root, lines, follow)?,
        },
        Command::Completion { target } => {
            let mut command = Args::command();
            if let Some(generator) = target.generator() {
                let mut buf = Vec::new();
                clap_complete::generate(generator, &mut command, "kanban", &mut buf);
                let script = String::from_utf8(buf).expect("clap_complete output should be utf8");
                let enhanced = match generator {
                    clap_complete::Shell::Zsh => enhance_zsh_completion(&script),
                    clap_complete::Shell::Bash => enhance_bash_completion(&script),
                    _ => script,
                };
                print!("{enhanced}");
            } else {
                println!("{COMPLETION_HELP}");
            }
        }
        Command::Validate { repo_root } => {
            let report = validate_repository(repo_root)?;
            if report.issues.is_empty() {
                println!("{}", theme.success("No validation issues found."));
            } else {
                for issue in report.issues {
                    println!(
                        "{} [{}] {}",
                        theme.path(issue.file_path.display()),
                        theme.warning(issue.rule),
                        issue.message
                    );
                }
            }
        }
        Command::Doctor { command } => match command {
            DoctorCommand::Show { repo_root } => {
                let findings = doctor_repository(repo_root)?;
                print_doctor_findings(&theme, &findings);
            }
            DoctorCommand::Fix { target, repo_root } => {
                run_doctor_fix_wizard(&theme, &repo_root, target.as_deref())?;
            }
        },
        Command::ListIds { kind, repo_root } => match kind {
            ListIdsKind::Sprints => {
                for id in list_sprint_names(repo_root)? {
                    println!("{id}");
                }
            }
            ListIdsKind::Stories => {
                for id in list_story_ids(repo_root)? {
                    println!("{id}");
                }
            }
            ListIdsKind::StoriesWithTitles => {
                for item in list_story_completion_items(repo_root)? {
                    let description = item.description.replace(['\t', '\n', '\r'], " ");
                    println!("{}\t{}", item.value, description);
                }
            }
            ListIdsKind::Epics => {
                for id in list_epic_ids(repo_root)? {
                    println!("{id}");
                }
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
