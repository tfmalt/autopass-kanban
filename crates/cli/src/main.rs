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
                    println!("{}", theme.success("Sprint rosters already up to date."));
                } else {
                    println!("{}", theme.success("Regenerated sprint rosters:"));
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
#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::prelude::*;
    #[allow(unused_imports)]
    use crate::{
        cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*,
        theme::*, web::*,
    };
    use clap::Parser;
    #[allow(unused_imports)]
    use kanban_core::*;
    use std::collections::BTreeMap;

    #[test]
    fn plain_theme_preserves_text_without_ansi_codes() {
        let theme = Theme::plain();

        assert_eq!(theme.status("blocked"), "blocked");
        assert_eq!(theme.id("US-F1-056"), "US-F1-056");
        assert!(!theme.status("done").contains("\x1b["));
    }

    #[test]
    fn color_theme_keeps_status_text_while_adding_ansi_codes() {
        let theme = Theme::color();
        let styled = theme.status("in-progress");

        assert!(styled.contains("\x1b["));
        assert!(styled.contains("in-progress"));
    }

    #[test]
    fn doctor_fix_preview_renders_key_old_and_new_values() {
        let theme = Theme::plain();
        let issue = DoctorIssue {
            severity: "info".to_string(),
            scope: "story.md".to_string(),
            file_path: None,
            story_id: None,
            sprint_name: None,
            rule: "invalid-timestamp:updated".to_string(),
            message: String::new(),
            suggestion: String::new(),
            fix_preview: Some(kanban_core::DoctorFixPreview {
                field_name: "updated".to_string(),
                old_value: "2026-05-31".to_string(),
                new_value: "2026-05-31T00:00:00+0200".to_string(),
            }),
            fix_kind: DoctorFixKind::Automatic,
            prompt: DoctorPrompt::None,
        };

        assert_eq!(
            format_doctor_fix_preview(&theme, &issue),
            "updated: 2026-05-31 -> 2026-05-31T00:00:00+0200"
        );
    }

    #[test]
    fn doctor_frontmatter_tokens_are_highlighted_with_color_theme() {
        let theme = Theme::color();
        let highlighted = highlight_frontmatter_tokens(
            &theme,
            "Frontmatter field \"updated\" must replace `2026-05-31`.",
        );

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("updated"));
        assert!(highlighted.contains("2026-05-31"));
    }

    #[test]
    fn sprint_overview_wraps_story_rows_to_terminal_width() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F1-999".to_string(),
                title: "Improve current sprint terminal rendering so story descriptions wrap responsively inside the detected table boundary".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F1-99".to_string()),
                epic_title: Some("Terminal Rendering".to_string()),
                assignee: "Ada Lovelace <ada@example.test>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S999.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-9-test/US-F1-999.md"),
                task_summary: Some(TaskSummary {
                    todo: 1,
                    in_progress: 2,
                    blocked: 3,
                    done: 4,
                }),
                task_count: 10,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S999.test".to_string(),
            headline: "terminal-wrapping".to_string(),
            sprint_goal: Some(
                "Keep sprint output useful without repeating implementation file paths.".to_string(),
            ),
            start_date: "2026-05-29".to_string(),
            end_date: "2026-06-12".to_string(),
            readme_path: PathBuf::from("delivery/sprints/S999.test.md"),
            readme_status: Some("active".to_string()),
            stories_by_status,
            blocked_work: vec![kanban_core::BlockedWorkItem {
                story_id: "US-F1-999".to_string(),
                story_title: "Improve current sprint terminal rendering so blocked work also wraps responsively".to_string(),
                task_id: Some("T-001".to_string()),
                task_title: Some("Verify narrow but supported terminal widths do not overflow".to_string()),
            }],
            warnings: Vec::new(),
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 80 }, &sprint);

        assert!(output.contains("S999 · Terminal Wrapping"));
        assert!(output.contains("Sprint Goal:"));
        assert!(!output.contains("README:"));
        assert!(output.contains("US-F1-999"));
        assert!(!output.contains('|'));
        for line in output.lines() {
            assert!(
                display_width(line) <= 80,
                "line exceeded 80 columns: {line}"
            );
        }
    }

    #[test]
    fn display_width_ignores_ansi_codes() {
        assert_eq!(display_width("\x1b[1;32mhello\x1b[0m"), 5);
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("\x1b[2m✓4\x1b[0m"), 2);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn header_band_fills_terminal_width() {
        let theme = Theme::plain();
        let sprint = SprintOverview {
            sprint_name: "S001.foundation".to_string(),
            headline: "foundation".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("delivery/sprints/S001.foundation.md"),
            readme_status: Some("active".to_string()),
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };

        for width in [80, 100, 120] {
            let mut output = String::new();
            push_sprint_header_band(&mut output, &theme, OutputLayout { width }, &sprint);
            let non_empty: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
            // First line = top separator, last line = bottom separator — both full-width.
            assert_eq!(
                display_width(non_empty[0]),
                width,
                "top separator at width {width}"
            );
            assert_eq!(
                display_width(non_empty[non_empty.len() - 1]),
                width,
                "bottom separator at width {width}"
            );
        }
    }

    #[test]
    fn progress_bar_scales_with_terminal_width() {
        let theme = Theme::plain();
        let bar_80 = render_progress_bar(&theme, 6, 4, 14, 80);
        let bar_120 = render_progress_bar(&theme, 6, 4, 14, 120);
        assert_eq!(display_width(&bar_80), 80 / 5 - 2);
        assert_eq!(display_width(&bar_120), 120 / 5 - 2);
        assert!(bar_80.starts_with("\u{e0b6}"));
        assert!(bar_80.ends_with("\u{e0b4}"));
    }

    #[test]
    fn progress_bar_uses_done_and_in_progress_status_colors() {
        let theme = Theme::color();
        let bar = render_progress_bar(&theme, 5, 3, 10, 100);

        assert!(
            bar.contains("\x1b[1;32m"),
            "done segment should be green: {bar}"
        );
        assert!(
            bar.contains("\x1b[1;34m"),
            "in-progress segment should be blue: {bar}"
        );
        assert!(
            bar.contains("\x1b[1;34;40m"),
            "in-progress boundary should use dark gray background: {bar}"
        );
        assert!(
            bar.contains("\x1b[90m\u{e0b4}"),
            "right cap should use dark gray foreground: {bar}"
        );
        assert_eq!(display_width(&bar), 100 / 5 - 2);
    }

    #[test]
    fn progress_bar_uses_eighth_block_resolution() {
        let plain = render_progress_bar(&Theme::plain(), 1, 0, 7, 100);
        assert!(
            plain.contains("▎"),
            "expected one-quarter boundary after cap columns: {plain}"
        );

        let colored = render_progress_bar(&Theme::color(), 1, 1, 7, 100);
        assert!(
            colored.contains("\x1b[1;32;44m▎"),
            "done to in-progress boundary should use green foreground and blue background: {colored}"
        );
    }

    #[test]
    fn sprint_progress_uses_story_points() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "done".to_string(),
            vec![StoryOverview {
                id: "US-F1-001".to_string(),
                title: "Completed high-value story".to_string(),
                status: "done".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("Test Epic".to_string()),
                assignee: "TBD".to_string(),
                story_points: "8".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-001.md"),
                task_summary: None,
                task_count: 0,
            }],
        );
        stories_by_status.insert(
            "todo".to_string(),
            vec![StoryOverview {
                id: "US-F1-002".to_string(),
                title: "Remaining smaller story".to_string(),
                status: "todo".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("Test Epic".to_string()),
                assignee: "TBD".to_string(),
                story_points: "2".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-002.md"),
                task_summary: None,
                task_count: 0,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);

        assert!(
            output.contains("◈8 / 10"),
            "progress line should use story points: {output}"
        );
        assert!(
            output.contains("80%"),
            "progress percentage should use story points: {output}"
        );
    }

    #[test]
    fn assignee_strips_email() {
        assert_eq!(
            extract_assignee_name("Geir Ivar Jerstad <g@v.no>"),
            "Geir Ivar Jerstad"
        );
        assert_eq!(
            extract_assignee_name("Thomas Malt <thomas.malt@vegvesen.no>"),
            "Thomas Malt"
        );
        assert_eq!(
            extract_assignee_name("Sondre Bjerkerud and Erik Itland"),
            "Sondre Bjerkerud and Erik Itland"
        );
        assert_eq!(extract_assignee_name("TBD"), "TBD");
    }

    #[test]
    fn task_symbols_replace_old_format() {
        let summary = TaskSummary {
            todo: 2,
            in_progress: 1,
            blocked: 0,
            done: 4,
        };
        let plain = format_compact_task_summary(Some(&summary));
        assert!(plain.contains("✓4"), "done symbol missing: {plain}");
        assert!(plain.contains("▶1"), "active symbol missing: {plain}");
        assert!(plain.contains("·2"), "todo symbol missing: {plain}");
        assert!(plain.contains("✗0"), "blocked symbol missing: {plain}");
        assert!(!plain.contains("T:"), "old T: format present: {plain}");
        assert!(!plain.contains("IP:"), "old IP: format present: {plain}");
    }

    #[test]
    fn story_status_rows_include_story_points() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "todo".to_string(),
            vec![StoryOverview {
                id: "US-F1-062".to_string(),
                title: "A larger story".to_string(),
                status: "todo".to_string(),
                epic_id: Some("EP-F1-06".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "13".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-062.md"),
                task_summary: None,
                task_count: 0,
            }],
        );
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F3-001".to_string(),
                title: "A smaller story".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F3-01".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "5".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-3/US-F3-001.md"),
                task_summary: None,
                task_count: 0,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);

        assert!(
            output.contains("US-F1-062 ◈13"),
            "story row should include story points: {output}"
        );
        assert!(
            output.contains("    · US-F1-062 ◈13"),
            "story row should be indented below the status header and prefixed with a bullet: {output}"
        );
        assert!(
            output.contains("○ todo   1 story   ◈13"),
            "todo header should include story point total: {output}"
        );
        assert!(
            output.contains("→ in-progress   1 story   ◈5"),
            "in-progress header should include story point total: {output}"
        );
        assert!(
            output.contains("US-F3-001  ◈5"),
            "single-digit story points should be right-aligned: {output}"
        );
    }

    #[test]
    fn story_status_rows_highlight_story_points() {
        let theme = Theme::color();
        let story = StoryOverview {
            id: "US-F1-002".to_string(),
            title: "A story in progress".to_string(),
            status: "in-progress".to_string(),
            epic_id: Some("EP-F1-01".to_string()),
            epic_title: Some("CLI".to_string()),
            assignee: "Someone <s@example.com>".to_string(),
            story_points: "3".to_string(),
            sprint: Some("S001.test".to_string()),
            relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-002.md"),
            task_summary: None,
            task_count: 0,
        };

        let label = format_colored_story_status_label(&theme, &story, 3);

        assert!(label.contains("\x1b[1;36mUS-F1-002\x1b[0m"));
        assert!(label.contains(" \x1b[1;33m◈3\x1b[0m"));
    }

    #[test]
    fn done_section_expands_in_overview() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "done".to_string(),
            vec![StoryOverview {
                id: "US-F1-001".to_string(),
                title: "A completed story".to_string(),
                status: "done".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "2".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-001.md"),
                task_summary: None,
                task_count: 0,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec![],
        };
        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);
        assert!(
            output.contains("✓ done   1 story   ◈2"),
            "done section header missing story points"
        );
        assert!(
            output.contains("A completed story"),
            "done story should be listed individually"
        );
    }

    #[test]
    fn zero_count_section_shows_single_muted_line() {
        let theme = Theme::plain();
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };
        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);
        assert!(output.contains("○ todo"), "todo section header missing");
        assert!(
            output
                .lines()
                .any(|line| line == "  ○ todo   0 stories   ◈0   · none"),
            "todo section should be inset by two spaces"
        );
        assert!(
            output.contains("none"),
            "none placeholder missing for empty section"
        );
    }

    #[test]
    fn sprint_sections_are_divided_and_inset() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F1-002".to_string(),
                title: "A story in progress".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("CLI".to_string()),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S001.test".to_string()),
                relative_path: PathBuf::from("delivery/backlog/phase-1/US-F1-002.md"),
                task_summary: None,
                task_count: 0,
            }],
        );
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: Some("Keep the overview readable.".to_string()),
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status,
            blocked_work: vec![],
            warnings: vec!["A warning line".to_string()],
        };

        let output = render_sprint_overview(&theme, OutputLayout { width: 100 }, &sprint);
        let divider = "─".repeat(100);

        assert!(
            output.lines().any(|line| line == divider),
            "section divider should span the full width without indentation"
        );
        assert!(
            output.lines().any(|line| line == "  A warning line"),
            "warning should be inset by two spaces"
        );
        assert!(
            output
                .lines()
                .any(|line| line == "  → in-progress   1 story   ◈3"),
            "status header should be inset by two spaces"
        );
    }

    #[test]
    fn sprint_header_title_uses_bright_color() {
        let theme = Theme::color();
        let sprint = SprintOverview {
            sprint_name: "S001.scaffolding".to_string(),
            headline: "scaffolding".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview_short(&theme, OutputLayout { width: 100 }, &sprint);

        assert!(
            output.contains("\x1b[1;36mS001 · Scaffolding\x1b[0m"),
            "sprint title should be highlighted with bright cyan: {output:?}"
        );
    }

    #[test]
    fn sprint_header_band_has_blank_lines_around_status_rows() {
        let theme = Theme::plain();
        let sprint = SprintOverview {
            sprint_name: "S001.test".to_string(),
            headline: "test".to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-30".to_string(),
            readme_path: PathBuf::from("README.md"),
            readme_status: None,
            stories_by_status: BTreeMap::new(),
            blocked_work: vec![],
            warnings: vec![],
        };

        let output = render_sprint_overview_short(&theme, OutputLayout { width: 100 }, &sprint);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 6, "header should only contain the header band");
        assert!(
            lines[1].is_empty(),
            "blank line should appear above the status rows"
        );
        assert!(
            lines[4].is_empty(),
            "blank line should appear below the status rows"
        );
    }

    #[test]
    fn command_repo_root_uses_subcommand_repo_root() {
        let repo_root = PathBuf::from("/tmp/kanban-repo");
        let command = Command::Sprint {
            command: SprintCommand::List {
                repo_root: repo_root.clone(),
            },
        };

        assert_eq!(command_repo_root(&command), Some(&repo_root));
    }

    #[test]
    fn sprint_show_without_name_parses_as_current_sprint() {
        let args = Args::try_parse_from(["kanban", "sprint", "show"]).unwrap();

        match args.command {
            Command::Sprint {
                command:
                    SprintCommand::Show {
                        name,
                        short,
                        repo_root,
                    },
            } => {
                assert_eq!(name, None);
                assert!(!short);
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn sprint_show_with_name_still_parses_named_sprint() {
        let args = Args::try_parse_from(["kanban", "sprint", "show", "S001.foundation"]).unwrap();

        match args.command {
            Command::Sprint {
                command:
                    SprintCommand::Show {
                        name,
                        short,
                        repo_root,
                    },
            } => {
                assert_eq!(name.as_deref(), Some("S001.foundation"));
                assert!(!short);
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn sprint_show_short_flag_parses() {
        let args = Args::try_parse_from(["kanban", "sprint", "show", "--short"]).unwrap();

        match args.command {
            Command::Sprint {
                command: SprintCommand::Show { short, .. },
            } => {
                assert!(short);
            }
            _ => panic!("unexpected command"),
        }
    }

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
    fn print_story_list_renders_scope_and_story_rows() {
        let theme = Theme::plain();
        let stories = vec![StoryOverview {
            id: "US-F1-010".to_string(),
            title: "CI pipeline with build and unit tests".to_string(),
            status: "in-progress".to_string(),
            epic_id: Some("EP-F1-01".to_string()),
            epic_title: Some("Platform".to_string()),
            assignee: "Ada Lovelace <ada@example.test>".to_string(),
            story_points: "3".to_string(),
            sprint: Some("S000.getting-started".to_string()),
            relative_path: PathBuf::from(
                "delivery/backlog/phase-1-scaffolding/01.some-epic/US-F1-010.md",
            ),
            task_summary: None,
            task_count: 0,
        }];

        let output = render_story_list(&theme, "active sprint (S000.getting-started)", &stories);

        assert!(output.contains("Stories: 1"));
        assert!(output.contains("Scope: active sprint (S000.getting-started)"));
        assert!(output.contains("US-F1-010 [in-progress] sprint=S000.getting-started"));
        assert!(output.contains("◈3"));
    }

    #[test]
    fn phase_overview_groups_stories_by_epic_and_status() {
        let theme = Theme::plain();
        let phase = PhaseOverview {
            phase: "F1".to_string(),
            stories: vec![
                StoryOverview {
                    id: "US-F1-010".to_string(),
                    title: "CI pipeline with build and unit tests".to_string(),
                    status: "todo".to_string(),
                    epic_id: Some("EP-F1-01".to_string()),
                    epic_title: Some("Platform".to_string()),
                    assignee: "Ada Lovelace <ada@example.test>".to_string(),
                    story_points: "3".to_string(),
                    sprint: Some("S000.getting-started".to_string()),
                    relative_path: PathBuf::from(
                        "delivery/backlog/phase-1-scaffolding/01.platform/US-F1-010.md",
                    ),
                    task_summary: Some(TaskSummary {
                        todo: 2,
                        in_progress: 0,
                        blocked: 0,
                        done: 1,
                    }),
                    task_count: 3,
                },
                StoryOverview {
                    id: "US-F1-011".to_string(),
                    title: "Preview story details in the terminal".to_string(),
                    status: "in-progress".to_string(),
                    epic_id: Some("EP-F1-01".to_string()),
                    epic_title: Some("Platform".to_string()),
                    assignee: "Grace Hopper <grace@example.test>".to_string(),
                    story_points: "5".to_string(),
                    sprint: None,
                    relative_path: PathBuf::from(
                        "delivery/backlog/phase-1-scaffolding/01.platform/US-F1-011.md",
                    ),
                    task_summary: Some(TaskSummary {
                        todo: 1,
                        in_progress: 2,
                        blocked: 0,
                        done: 0,
                    }),
                    task_count: 3,
                },
                StoryOverview {
                    id: "US-F1-020".to_string(),
                    title: "Sync sprint rosters from story metadata".to_string(),
                    status: "done".to_string(),
                    epic_id: Some("EP-F1-02".to_string()),
                    epic_title: Some("Planning".to_string()),
                    assignee: "TBD".to_string(),
                    story_points: "2".to_string(),
                    sprint: Some("S001.foundation".to_string()),
                    relative_path: PathBuf::from(
                        "delivery/backlog/phase-1-scaffolding/02.planning/US-F1-020.md",
                    ),
                    task_summary: None,
                    task_count: 0,
                },
            ],
        };

        let output = render_phase_overview(&theme, OutputLayout { width: 100 }, &phase);

        assert!(output.contains("F1 · Phase Overview"));
        assert!(output.contains("3 stories"));
        assert!(output.contains("Progress:"));
        assert!(output.contains("◈2 / 10"));
        assert!(output.contains("20%"));
        assert!(output.contains("◈0 drafted"));
        assert!(output.contains("◈3 planned"));
        assert!(output.contains("◈5 in progress"));
        assert!(output.contains("◈2 done"));
        assert!(output.contains("2 epics"));
        assert!(output.contains("◈10 total"));
        assert!(output.contains("EP-F1-01  Platform   2 stories   ◈8"));
        assert!(output.contains("○ todo   1 story   ◈3"));
        assert!(output.contains("→ in-progress   1 story   ◈5"));
        assert!(output.contains("✓ done   1 story   ◈2"));
        assert!(output.contains("S000.getting-started"));
        assert!(output.contains("~"));
        assert!(output.contains("Ada Lovelace"));
        assert!(output.contains("Grace Hopper"));
        assert!(output.contains("Sync sprint rosters from story metadata"));
        for line in output.lines() {
            assert!(
                display_width(line) <= 100,
                "line exceeded 100 columns: {line}"
            );
        }
    }

    #[test]
    fn story_details_render_terminal_formatted_markdown() {
        let theme = Theme::plain();
        let details = StoryDetails {
            story: StoryOverview {
                id: "US-F1-010".to_string(),
                title: "CI pipeline with build and unit tests".to_string(),
                status: "in-progress".to_string(),
                epic_id: Some("EP-F1-01".to_string()),
                epic_title: Some("Plattforminfrastruktur".to_string()),
                assignee: "Ada Lovelace <ada@example.test>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S000.getting-started".to_string()),
                relative_path: PathBuf::from(
                    "delivery/backlog/phase-1-scaffolding/01.some-epic/US-F1-010.md",
                ),
                task_summary: Some(TaskSummary {
                    todo: 1,
                    in_progress: 1,
                    blocked: 0,
                    done: 2,
                }),
                task_count: 4,
            },
            story_file_path: PathBuf::from(
                "delivery/backlog/phase-1-scaffolding/01.plattforminfrastruktur/US-F1-010.md",
            ),
            task_file_path: None,
            epic_id: Some("EP-F1-01".to_string()),
            epic_title: Some("Plattforminfrastruktur".to_string()),
            work_started: Some("2026-05-21T00:00:00+0200".to_string()),
            work_done: None,
            story_statement: Some(
                "As a developer\n\n- I need **formatted** story output".to_string(),
            ),
            acceptance_criteria: Some(
                "Scenario: Show a story\nGiven a story exists\nWhen I run the command\nThen the story is formatted".to_string(),
            ),
            definition_of_done: Some("- [ ] Run `cargo test`".to_string()),
            notes_and_open_questions: Some(
                "| Risk | Mitigation |\n| --- | --- |\n| Raw markdown | Render terminal tables |"
                    .to_string(),
            ),
            tasks: vec![kanban_core::Task {
                id: "TASK-US-F1-010-001".to_string(),
                title: "Build story renderer".to_string(),
                status: "In Progress".to_string(),
                normalized_status: "in-progress".to_string(),
                tags: vec!["cli".to_string()],
                description: "Wire command output".to_string(),
            }],
        };

        let output = render_story_details(&theme, OutputLayout { width: 100 }, &details);

        assert!(output.contains("US-F1-010 · CI pipeline with build and unit tests"));
        assert!(output.contains("Overview"));
        assert!(output.contains("Field"));
        assert!(output.contains("Value"));
        assert!(output.contains("Scenario: Show a story"));
        assert!(output.contains("Given a story exists"));
        assert!(output.contains("☐ Run cargo test"));
        assert!(output.contains("Risk"));
        assert!(output.contains("Mitigation"));
        assert!(output.contains("1 Scaffolding"));
        assert!(output.contains("EP-F1-01 Plattforminfrastruktur"));
        assert!(output.contains("phase-1-scaffolding/01.plattforminfrastruktur/US-F1-010.md"));
        assert!(output.contains("2026-05-21T00:00:00+0200"));
        assert!(output.contains("TASK-US-F1-010-001"));
        assert!(output.contains("→ in-progress"));
        assert!(output.contains("Build story renderer - Wire command output"));
        assert!(!output.contains("Story:"));
        assert!(!output.contains("Task file"));
        assert!(!output.contains("delivery/backlog/"));
        assert!(!output.contains("| Risk | Mitigation |"));
        assert!(!output.contains("- [ ] Run `cargo test`"));
    }

    #[test]
    fn fenced_gherkin_blocks_are_syntax_highlighted() {
        let theme = Theme::color();
        let mut output = String::new();

        push_terminal_markdown(
            &mut output,
            &theme,
            100,
            "```gherkin\nGiven a developer opens a pull request\nWhen the pipeline runs\nThen the status is visible\n```",
        );

        assert!(output.contains("  │ "));
        assert!(output.contains("\x1b[1mGiven\x1b[0m a developer opens a pull request"));
        assert!(output.contains("\x1b[1mWhen\x1b[0m the pipeline runs"));
        assert!(output.contains("\x1b[1mThen\x1b[0m the status is visible"));
    }

    #[test]
    fn story_list_command_reuses_story_repo_root() {
        let repo_root = PathBuf::from("/tmp/kanban-repo");
        let command = Command::Story {
            command: StoryCommand::List {
                current: false,
                all: false,
                next: false,
                sprint: None,
                repo_root: repo_root.clone(),
            },
        };

        assert_eq!(command_repo_root(&command), Some(&repo_root));
    }

    #[test]
    fn story_update_parses_direct_frontmatter_values() {
        let args = Args::try_parse_from([
            "kanban",
            "story",
            "update",
            "US-F1-099",
            "--story-points",
            "5",
            "--status",
            "ready",
        ])
        .unwrap();

        match args.command {
            Command::Story {
                command:
                    StoryCommand::Update {
                        id,
                        story_points,
                        status,
                        repo_root,
                        ..
                    },
            } => {
                assert_eq!(id, "US-F1-099");
                assert_eq!(story_points, Some(Some("5".to_string())));
                assert_eq!(status, Some(Some("ready".to_string())));
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn story_update_parses_bare_frontmatter_option_as_prompt() {
        let args =
            Args::try_parse_from(["kanban", "story", "update", "US-F1-099", "--story-points"])
                .unwrap();

        match args.command {
            Command::Story {
                command:
                    StoryCommand::Update {
                        story_points,
                        status,
                        ..
                    },
            } => {
                assert_eq!(story_points, Some(None));
                assert_eq!(status, None);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn doctor_show_subcommand_parses() {
        let args = Args::try_parse_from(["kanban", "doctor", "show"]).unwrap();

        match args.command {
            Command::Doctor {
                command: DoctorCommand::Show { repo_root },
            } => assert_eq!(repo_root, PathBuf::from(".")),
            _ => panic!("unexpected command"),
        }
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

    #[test]
    fn doctor_fix_current_parses() {
        let args = Args::try_parse_from(["kanban", "doctor", "fix", "current"]).unwrap();

        match args.command {
            Command::Doctor {
                command: DoctorCommand::Fix { target, repo_root },
            } => {
                assert_eq!(target.as_deref(), Some("current"));
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn doctor_fix_story_parses() {
        let args = Args::try_parse_from(["kanban", "doctor", "fix", "US-F1-053"]).unwrap();

        match args.command {
            Command::Doctor {
                command: DoctorCommand::Fix { target, repo_root },
            } => {
                assert_eq!(target.as_deref(), Some("US-F1-053"));
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn web_start_command_parses_flags() {
        let args = Args::try_parse_from(["kanban", "web", "start", "--dev", "--open", "/tmp/repo"])
            .unwrap();

        match args.command {
            Command::Web {
                command:
                    WebCommand::Start {
                        foreground,
                        open,
                        dev,
                        build,
                        repo_root,
                    },
            } => {
                assert!(!foreground);
                assert!(open);
                assert!(dev);
                assert!(!build);
                assert_eq!(repo_root, PathBuf::from("/tmp/repo"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn web_restart_command_parses_build_flag() {
        let args = Args::try_parse_from(["kanban", "web", "restart", "--build"]).unwrap();

        match args.command {
            Command::Web {
                command:
                    WebCommand::Restart {
                        open,
                        dev,
                        build,
                        repo_root,
                    },
            } => {
                assert!(!open);
                assert!(!dev);
                assert!(build);
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn web_log_command_parses_lines_and_follow() {
        let args = Args::try_parse_from([
            "kanban",
            "web",
            "log",
            "--lines",
            "50",
            "--follow",
            "/tmp/repo",
        ])
        .unwrap();

        match args.command {
            Command::Web {
                command:
                    WebCommand::Log {
                        lines,
                        follow,
                        repo_root,
                    },
            } => {
                assert_eq!(lines, Some(50));
                assert!(follow);
                assert_eq!(repo_root, PathBuf::from("/tmp/repo"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn command_repo_root_uses_web_subcommand_repo_root() {
        let repo_root = PathBuf::from("/tmp/kanban-repo");
        let command = Command::Web {
            command: WebCommand::Status {
                repo_root: repo_root.clone(),
            },
        };

        assert_eq!(command_repo_root(&command), Some(&repo_root));
    }

    #[test]
    fn web_runtime_paths_live_under_kanban_run() {
        let paths = web_runtime_paths(Path::new("/tmp/repo"));

        assert_eq!(paths.run_dir, PathBuf::from("/tmp/repo/.kanban/run"));
        assert_eq!(
            paths.pid_file,
            PathBuf::from("/tmp/repo/.kanban/run/web.pid")
        );
        assert_eq!(
            paths.port_file,
            PathBuf::from("/tmp/repo/.kanban/run/web.port")
        );
        assert_eq!(
            paths.log_file,
            PathBuf::from("/tmp/repo/.kanban/run/web.log")
        );
    }

    #[test]
    fn web_already_running_error_uses_icon_and_aligned_guidance() {
        let output = render_web_already_running_error(&Theme::plain(), 77322, 100);

        assert_eq!(
            output,
            "✖ Error: kanban web is already running with PID 77322.\n  Use `kanban web status` or `kanban web restart`.\n"
        );
    }

    #[test]
    fn web_already_running_error_wraps_with_hanging_indent() {
        let output = render_web_already_running_error(&Theme::plain(), 77322, 48);

        for line in output.lines().skip(1) {
            assert!(line.starts_with("  "), "line was not indented: {line}");
        }
        assert!(output.contains("\n  77322.\n"));
        assert!(output.contains("\n  `kanban web restart`.\n"));
    }

    #[test]
    fn web_already_running_error_uses_theme_colors_for_error_and_commands() {
        let output = render_web_already_running_error(&Theme::color(), 77322, 100);

        assert!(output.contains("\x1b[1;31m✖\x1b[0m"));
        assert!(output.contains("\x1b[1;31mError:\x1b[0m"));
        assert!(output.contains("\x1b[1;34m`kanban web status`\x1b[0m"));
        assert!(output.contains("\x1b[1;34m`kanban web restart`\x1b[0m"));
    }

    #[test]
    fn web_port_fallback_warning_reports_actual_url() {
        let output = render_web_port_fallback_warning(&Theme::plain(), "127.0.0.1", 3000, 3001);

        assert_eq!(
            output,
            "Warning: another service is already using http://127.0.0.1:3000; starting kanban web UI on http://127.0.0.1:3001 instead."
        );
    }

    #[test]
    fn web_start_specs_select_production_or_dev_command() {
        let repo_root = Path::new("/tmp/repo");

        let production = build_web_start_command_spec(repo_root, false);
        assert_eq!(production.program, "node");
        assert_eq!(production.cwd, PathBuf::from("/tmp/repo"));
        assert!(production.args[0].ends_with("tools/kanban-web/dist/server/index.js"));

        let dev = build_web_start_command_spec(repo_root, true);
        assert_eq!(dev.program, "npm");
        assert_eq!(dev.cwd, PathBuf::from("/tmp/repo"));
        assert_eq!(dev.args[0], "--prefix");
        assert!(dev.args[1].ends_with("tools/kanban-web"));
        assert_eq!(&dev.args[2..], ["run", "dev:server"]);
    }
}
