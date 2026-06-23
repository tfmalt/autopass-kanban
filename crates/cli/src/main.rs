mod cli;
mod completion;
mod doctor_cli;
mod json_out;
mod layout;
mod prompt;
mod render;
mod self_manage;
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
    #[cfg(unix)]
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
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*,
    self_manage::*, theme::*, web::*,
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

pub(crate) fn render_version_line(theme: &Theme) -> String {
    format!(
        "{} {}",
        theme.brand(),
        theme.version(env!("CARGO_PKG_VERSION"))
    )
}

pub(crate) fn render_no_args_help_output(theme: &Theme) -> Result<String> {
    let mut command = Args::command();
    let help = render_styled_output(command.render_help(), theme.color);
    Ok(format!("{}\n{help}\n", render_version_line(theme)))
}

pub(crate) fn command_requires_git_repository(command: &Command) -> bool {
    !matches!(command, Command::Upgrade { .. } | Command::Uninstall { .. })
}

pub(crate) fn command_git_requirement_path(command: &Command) -> &Path {
    command_repo_root(command)
        .map(PathBuf::as_path)
        .unwrap_or_else(|| Path::new("."))
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

    let path = command_git_requirement_path(command);
    if is_git_repository(path) {
        None
    } else {
        Some(git_repository_requirement_message(path))
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

fn main() -> Result<()> {
    let raw_args = normalize_args(std::env::args_os().collect::<Vec<_>>());
    if raw_args.len() == 2 && matches!(raw_args[1].to_str(), Some("--version" | "-V")) {
        let theme = Theme::for_stdout(ColorMode::Auto);
        println!("{}", render_version_line(&theme));
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
        std::process::exit(emit_json(&args.command));
    }

    if let Some(message) = command_git_requirement_error(&args.command) {
        bail!(message);
    }

    let theme = theme_for_command(&args.command);

    match args.command {
        Command::Init {
            repo_root,
            no_sprints,
            no_epics,
            no_phases,
        } => {
            let features = if no_sprints || no_epics || no_phases {
                Some(FeaturesConfig {
                    phases: !no_phases,
                    sprints: !no_sprints,
                    epics: !no_epics,
                })
            } else {
                None
            };
            let result = init_config_with_features(repo_root, features)?;
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
                ensure_sprints_enabled(&repo_root)?;
                let sprint = summarize_current_sprint(repo_root)?;
                print_sprint_overview(&theme, OutputLayout::for_stdout()?, &sprint);
            }
            SprintCommand::List { repo_root } => {
                ensure_sprints_enabled(&repo_root)?;
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
                ensure_sprints_enabled(&repo_root)?;
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
                ensure_sprints_enabled(&repo_root)?;
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
                ensure_sprints_enabled(&repo_root)?;
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
                ensure_sprints_enabled(&repo_root)?;
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
                ensure_phases_enabled(&repo_root)?;
                let phase = summarize_phase(repo_root, &phase)?;
                print_phase_overview(&theme, OutputLayout::for_stdout()?, &phase);
            }
        },
        Command::Epic { command } => match command {
            EpicCommand::Show { id, repo_root } => {
                ensure_epics_enabled(&repo_root)?;
                match find_epic(repo_root, &id)? {
                    Some(details) => {
                        print_epic_details(&theme, OutputLayout::for_stdout()?, &details)
                    }
                    None => println!("{} {id}", theme.warning("Epic not found:")),
                }
            }
            EpicCommand::Update {
                id,
                priority,
                repo_root,
            } => {
                ensure_epics_enabled(&repo_root)?;
                let mut updates = Vec::new();
                match priority {
                    Some(Some(value)) => updates.push(("priority".to_string(), value)),
                    Some(None) => {
                        let epic = find_epic_with_source(&repo_root, &id)?
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Epic not found: {}",
                                    id.trim().to_ascii_uppercase()
                                )
                            })?
                            .1;
                        let default = epic
                            .frontmatter
                            .get("priority")
                            .cloned()
                            .unwrap_or_default();
                        let value = prompt_with_default("priority", &default)?;
                        updates.push(("priority".to_string(), value));
                    }
                    None => {}
                }

                if updates.is_empty() {
                    bail!("No epic frontmatter fields were provided.");
                }

                let result = update_epic_frontmatter(&repo_root, &id, &updates)?;
                println!(
                    "{} {} ({})",
                    theme.success("Updated"),
                    theme.id(&result.epic_id),
                    result.updated_fields.join(", ")
                );
                println!(
                    "{} {}",
                    theme.label("Epic:"),
                    theme.path(result.epic_path.display())
                );
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
                } else {
                    let config = kanban_core::load_kanban_config(&repo_root)?;
                    let sprints_enabled = config.features().sprints;
                    if next {
                        if !sprints_enabled {
                            ensure_sprints_enabled(&repo_root)?;
                        }
                        let (sprint_name, stories) = list_next_sprint_stories(repo_root)?;
                        (format!("next sprint ({sprint_name})"), stories)
                    } else if let Some(sprint_name) = sprint {
                        if !sprints_enabled {
                            ensure_sprints_enabled(&repo_root)?;
                        }
                        (
                            format!("sprint {sprint_name}"),
                            list_stories_in_sprint(repo_root, &sprint_name)?,
                        )
                    } else if !sprints_enabled {
                        ("all stories".to_string(), list_all_stories(repo_root)?)
                    } else {
                        let (sprint_name, stories) = list_current_sprint_stories(repo_root)?;
                        let label = if current {
                            format!("current sprint ({sprint_name})")
                        } else {
                            format!("active sprint ({sprint_name})")
                        };
                        (label, stories)
                    }
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
            StoryCommand::Delete { id, repo_root } => {
                let result = delete_story(repo_root, &id)?;
                println!(
                    "{} {}",
                    theme.success("Deleted"),
                    theme.id(&result.story_id)
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
                if let Some(sprint_name) = result.sprint_name {
                    println!("{} {}", theme.label("Updated sprint:"), sprint_name);
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
                priority,
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
                    ("priority", priority),
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
            TaskCommand::Show {
                story_id,
                repo_root,
            } => {
                let details = list_tasks_for_story(repo_root, &story_id)?
                    .ok_or_else(|| anyhow::anyhow!("Story not found: {story_id}"))?;
                print!(
                    "{}",
                    render_task_list(
                        &theme,
                        OutputLayout::for_stdout()?,
                        &details.story_id,
                        details.task_file_path.as_deref(),
                        &details.tasks,
                    )
                );
            }
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
            TaskCommand::Delete {
                story_id,
                task_id,
                repo_root,
            } => {
                let result = delete_task_from_story(repo_root, &story_id, &task_id)?;
                println!(
                    "{} {} from {}",
                    theme.success("Deleted"),
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
            WebCommand::Serve {
                repo_root,
                host,
                port,
            } => kanban_web_server::serve_blocking(kanban_web_server::WebServeOptions {
                repo_root,
                host,
                port,
            })?,
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
        Command::Uninstall {
            prefix,
            skills_dir,
            yes,
            dry_run,
            quiet,
        } => run_uninstall(UninstallOptions {
            prefix,
            skills_dir,
            yes,
            dry_run,
            quiet,
        })?,
        Command::Upgrade {
            prefix,
            skills_dir,
            no_skills,
            yes,
            force,
            dry_run,
            quiet,
        } => run_upgrade(UpgradeOptions {
            prefix,
            skills_dir,
            no_skills,
            yes,
            force,
            dry_run,
            quiet,
        })?,
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
        Command::Report { command } => match command {
            ReportCommand::Wbs { repo_root } => {
                let stories = list_all_stories(&repo_root)?;
                let sprints = summarize_sprints(&repo_root)?;
                let current = summarize_current_sprint(&repo_root)
                    .ok()
                    .map(|s| s.sprint_name);
                let dto = ReportWbsDto::build(&stories, &sprints, current.as_deref());

                println!(
                    "{}  {}",
                    theme.heading("WBS Report"),
                    theme.paint(Style::Muted, &dto.generated_at),
                );
                println!(
                    "  {}  {}",
                    theme.label("Stories:"),
                    theme.count(format!("{}", dto.stories.len()))
                );
                println!(
                    "  {}  {}",
                    theme.label("Sprints:"),
                    theme.count(format!("{}", dto.sprints.len()))
                );
                println!(
                    "  {}  {}",
                    theme.label("Remaining points:"),
                    theme.story_points(format_story_points(dto.velocity.remaining_points as usize))
                );
                if let Some(est) = dto.velocity.estimated_sprints_remaining {
                    println!(
                        "  {}  {:.1} sprints  (avg {:.1} pts/sprint over {} completed sprints)",
                        theme.label("Estimated remaining:"),
                        est,
                        dto.velocity.avg_points_per_sprint,
                        dto.velocity.completed_sprint_count,
                    );
                } else {
                    println!(
                        "  {}  {}",
                        theme.label("Estimated remaining:"),
                        theme.paint(Style::Muted, "no velocity data yet")
                    );
                }
                println!();
                println!(
                    "{}",
                    theme.paint(Style::Muted, "To generate an Excel report, run:")
                );
                println!(
                    "  {} --format json | python3 ../autopass-kanban/scripts/wbs_report.py \\",
                    theme.id("kanban report wbs")
                );
                println!(
                    "    {} delivery/backlog/2026-03-31.autopass_ip_2.0_wbs.xlsx \\",
                    theme.id("--template")
                );
                println!(
                    "    {} delivery/backlog/wbs_report.xlsx",
                    theme.id("--output")
                );
            }
            ReportCommand::Forecast { repo_root } => {
                let stories = list_all_stories(&repo_root)?;
                let sprints = summarize_sprints(&repo_root)?;
                let current = summarize_current_sprint(&repo_root)
                    .ok()
                    .map(|s| s.sprint_name);
                let dto = ReportForecastDto::build(&stories, &sprints, current.as_deref());

                println!(
                    "{}  {}",
                    theme.heading("Forecast"),
                    theme.paint(Style::Muted, &dto.generated_at),
                );
                println!(
                    "  {}  {}",
                    theme.label("Remaining points:"),
                    theme.story_points(format_story_points(dto.remaining_points as usize)),
                );
                println!(
                    "  {}  {:.1} pts/day over {} observed workdays ({})",
                    theme.label("Throughput:"),
                    dto.throughput.average,
                    dto.throughput.observed_day_count,
                    dto.confidence,
                );
                if let Some(date) = dto.completion.p80_date.as_deref() {
                    println!(
                        "  {}  P50 {}  /  P80 {}  /  P90 {}",
                        theme.label("Completion:"),
                        dto.completion.p50_date.as_deref().unwrap_or("-"),
                        date,
                        dto.completion.p90_date.as_deref().unwrap_or("-"),
                    );
                } else {
                    println!(
                        "  {}  {}",
                        theme.label("Completion:"),
                        theme.paint(Style::Muted, "no throughput data yet"),
                    );
                }
            }
        },
        Command::Features { command } => match command {
            FeaturesCommand::List { repo_root } => {
                let config = load_kanban_config(repo_root)?;
                let features = config.features();
                let mut lines = Vec::new();
                for (name, enabled) in [
                    ("phases", features.phases),
                    ("sprints", features.sprints),
                    ("epics", features.epics),
                ] {
                    let status = if enabled { "on" } else { "off" };
                    lines.push(format!("{name:8} {status}"));
                }
                println!("{}", theme.heading("Features"));
                for line in lines {
                    println!("  {line}");
                }
            }
            FeaturesCommand::Enable { feature, repo_root } => {
                let key = match feature {
                    FeatureName::Sprints => "features.sprints",
                    FeatureName::Epics => "features.epics",
                    FeatureName::Phases => "features.phases",
                };
                let result = set_config_value(&repo_root, key, "true")?;
                println!(
                    "{} {} = {}",
                    theme.success("Enabled"),
                    theme.id(&result.key),
                    result.value
                );
            }
            FeaturesCommand::Disable { feature, repo_root } => {
                let key = match feature {
                    FeatureName::Sprints => "features.sprints",
                    FeatureName::Epics => "features.epics",
                    FeatureName::Phases => "features.phases",
                };
                let result = set_config_value(&repo_root, key, "false")?;
                println!(
                    "{} {} = {}",
                    theme.success("Disabled"),
                    theme.id(&result.key),
                    result.value
                );
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
        Command::ListTaskIds {
            story_id,
            repo_root,
        } => {
            if let Some(details) = find_story(repo_root, &story_id)? {
                for task in details.tasks {
                    println!("{}", task.id);
                }
            }
        }
    }

    Ok(())
}

fn ensure_feature_enabled(
    repo_root: impl AsRef<Path>,
    feature: &str,
    config: &kanban_core::FeaturesConfig,
) -> Result<()> {
    let enabled = match feature {
        "sprints" => config.sprints,
        "epics" => config.epics,
        "phases" => config.phases,
        _ => true,
    };
    if !enabled {
        bail!(
            "Feature '{feature}' is disabled in .kanban/settings.json. Run `kanban features enable {feature}` to re-enable it. (repo: {})",
            repo_root.as_ref().display()
        );
    }
    Ok(())
}

fn ensure_sprints_enabled(repo_root: impl AsRef<Path>) -> Result<()> {
    let config = kanban_core::load_kanban_config(repo_root.as_ref())?;
    ensure_feature_enabled(repo_root, "sprints", &config.features())
}

fn ensure_epics_enabled(repo_root: impl AsRef<Path>) -> Result<()> {
    let config = kanban_core::load_kanban_config(repo_root.as_ref())?;
    ensure_feature_enabled(repo_root, "epics", &config.features())
}

fn ensure_phases_enabled(repo_root: impl AsRef<Path>) -> Result<()> {
    let config = kanban_core::load_kanban_config(repo_root.as_ref())?;
    ensure_feature_enabled(repo_root, "phases", &config.features())
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
