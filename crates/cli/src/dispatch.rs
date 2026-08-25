use anyhow::Result;
use clap::CommandFactory;
use std::path::Path;

use crate::cli::{
    Args, COMPLETION_HELP, Command, CompletionTarget, ConfigCommand, DoctorCommand, EpicCommand,
    FeatureName, FeaturesCommand, ListIdsKind, PhaseCommand, ReportCommand, SprintCommand,
    StoryCommand, TaskCommand, WebCommand, completion_target_label, list_ids_kind_label,
};
use crate::completion::{enhance_bash_completion, enhance_zsh_completion};
use crate::doctor_cli::{run_doctor_fix_non_interactive, run_doctor_fix_wizard};
use crate::json_out::json_story_frontmatter_updates;
use crate::ops::{build_create_sprint_input_from_flags, resolve_story_list_scope};
use crate::outcome::{CommandOutcome, FeatureToggleAction};
use crate::prompt::{
    open_markdown_in_editor, open_story_markdown_in_editor, prompt_create_sprint,
    prompt_with_default, sprint_frontmatter_update_value, story_frontmatter_update_value,
};
use crate::self_manage::{UninstallOptions, UpgradeOptions, run_uninstall, run_upgrade};
use crate::theme::Theme;
use crate::web::{
    WebRestartDto, print_web_log, print_web_status, start_web, stop_web, web_log_json,
    web_start_json, web_status_json, web_stop_json,
};
use chrono::NaiveDate;
use kanban_core::{
    CompletionDto, CreateStoryInput, FeaturesConfig, KanbanError, KanbanErrorBody, KanbanErrorCode,
    ListIdItemDto, ReportForecastDto, ReportWbsDto, WorkingCalendar, add_task_to_story,
    config_show_value, create_sprint, create_story, delete_story, delete_task_from_story,
    doctor_repository, find_epic_with_source, find_story, find_story_with_source, get_config_json,
    get_config_value, init_config_with_features, list_all_epic_overviews, list_all_stories,
    list_epic_ids, list_sprint_names, list_story_completion_items, list_story_ids,
    list_tasks_for_story, load_kanban_config, move_story_to_status_with_assignee,
    plan_story_into_sprint, read_story_file, rollover_sprint, set_config_value,
    story_markdown_file, suggested_sprint_dates, summarize_current_sprint, summarize_phase,
    summarize_sprint, summarize_sprints, sync_sprint_rosters, update_epic_frontmatter,
    update_sprint_frontmatter, update_story_frontmatter, update_task_in_story, validate_repository,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum DispatchMode {
    Human,
    Json,
}

/// Build the error returned when a feature gate blocks a command.
///
/// The human path restores the pre-refactor message (with the `(repo: ...)`
/// suffix); the JSON path keeps the typed `KanbanError::feature_disabled`
/// message the JSON envelope contract tests assert on.
fn feature_disabled_error(mode: DispatchMode, feature: &str, repo_root: &Path) -> anyhow::Error {
    match mode {
        DispatchMode::Human => anyhow::anyhow!(
            "Feature '{feature}' is disabled in .kanban/settings.json. Run `kanban features enable {feature}` to re-enable it. (repo: {})",
            repo_root.display()
        ),
        DispatchMode::Json => KanbanError::feature_disabled(feature).into(),
    }
}

fn report_sprints(repo_root: &Path) -> Result<Vec<kanban_core::SprintOverview>> {
    let config = load_kanban_config(repo_root)?;
    if config.features().sprints {
        summarize_sprints(repo_root)
    } else {
        Ok(Vec::new())
    }
}

fn report_current_sprint_name(
    repo_root: &Path,
    sprints: &[kanban_core::SprintOverview],
) -> Option<String> {
    if sprints.is_empty() {
        None
    } else {
        summarize_current_sprint(repo_root)
            .ok()
            .map(|s| s.sprint_name)
    }
}

fn ensure_sprints_enabled(mode: DispatchMode, repo_root: &Path) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().sprints {
        return Err(feature_disabled_error(mode, "sprints", repo_root));
    }
    Ok(())
}

fn ensure_epics_enabled(mode: DispatchMode, repo_root: &Path) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().epics {
        return Err(feature_disabled_error(mode, "epics", repo_root));
    }
    Ok(())
}

fn ensure_phases_enabled(mode: DispatchMode, repo_root: &Path) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().phases {
        return Err(feature_disabled_error(mode, "phases", repo_root));
    }
    Ok(())
}

pub(crate) fn execute_command(
    command: &Command,
    mode: DispatchMode,
    theme: &Theme,
) -> Result<CommandOutcome> {
    Ok(match command {
        Command::Init {
            repo_root,
            no_sprints,
            no_epics,
            no_phases,
        } => {
            let features = if *no_sprints || *no_epics || *no_phases {
                Some(FeaturesConfig {
                    phases: !*no_phases,
                    sprints: !*no_sprints,
                    epics: !*no_epics,
                })
            } else {
                None
            };
            CommandOutcome::Init(init_config_with_features(repo_root, features)?)
        }
        Command::Config { command } => match command {
            ConfigCommand::Show { repo_root } => {
                CommandOutcome::ConfigShow(get_config_json(repo_root)?)
            }
            ConfigCommand::Get { key, repo_root } => CommandOutcome::ConfigGet {
                key: key.clone(),
                value: get_config_value(repo_root, key)?,
            },
            ConfigCommand::Set {
                key,
                value,
                repo_root,
            } => CommandOutcome::ConfigSet(set_config_value(repo_root, key, value)?),
        },
        Command::Features { command } => match command {
            FeaturesCommand::List { repo_root } => {
                let config = load_kanban_config(repo_root)?;
                let features = config.features();
                CommandOutcome::FeaturesList {
                    phases: features.phases,
                    sprints: features.sprints,
                    epics: features.epics,
                }
            }
            FeaturesCommand::Enable { feature, repo_root } => {
                let key = feature_key(*feature);
                CommandOutcome::FeatureToggle {
                    action: FeatureToggleAction::Enable,
                    result: set_config_value(repo_root, key, "true")?,
                }
            }
            FeaturesCommand::Disable { feature, repo_root } => {
                let key = feature_key(*feature);
                CommandOutcome::FeatureToggle {
                    action: FeatureToggleAction::Disable,
                    result: set_config_value(repo_root, key, "false")?,
                }
            }
        },
        Command::Completion { target } => CommandOutcome::Completion(completion_output(*target)),
        Command::ListIds { kind, repo_root } => CommandOutcome::ListIds {
            kind: list_ids_kind_label(*kind),
            items: list_id_items(*kind, repo_root)?,
        },
        Command::ListTaskIds {
            story_id,
            repo_root,
        } => {
            let details = find_story(repo_root, story_id)?;
            let items = details
                .map(|details| {
                    details
                        .tasks
                        .into_iter()
                        .map(|task| ListIdItemDto::value(task.id))
                        .collect()
                })
                .unwrap_or_default();
            CommandOutcome::ListIds {
                kind: "tasks",
                items,
            }
        }
        Command::Sprint { command } => match command {
            SprintCommand::Current { repo_root } => {
                ensure_sprints_enabled(mode, repo_root)?;
                CommandOutcome::SprintOverview {
                    kind: "sprint.current",
                    sprint: summarize_current_sprint(repo_root)?,
                    short: false,
                }
            }
            SprintCommand::List { repo_root } => {
                ensure_sprints_enabled(mode, repo_root)?;
                CommandOutcome::SprintList {
                    sprints: summarize_sprints(repo_root)?,
                    current_name: summarize_current_sprint(repo_root)
                        .ok()
                        .map(|sprint| sprint.sprint_name),
                }
            }
            SprintCommand::Show {
                name,
                short,
                repo_root,
            } => {
                ensure_sprints_enabled(mode, repo_root)?;
                let sprint = match name {
                    Some(name) => summarize_sprint(repo_root, name)?,
                    None => summarize_current_sprint(repo_root)?,
                };
                CommandOutcome::SprintOverview {
                    kind: "sprint.show",
                    sprint,
                    short: *short,
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
                if mode == DispatchMode::Human {
                    ensure_sprints_enabled(mode, repo_root)?;
                }
                let any_flag =
                    number.is_some() || headline.is_some() || start.is_some() || end.is_some();
                if mode == DispatchMode::Json && !*non_interactive && !any_flag {
                    return Ok(CommandOutcome::Error {
                        kind: "sprint.create",
                        body: KanbanErrorBody::new(
                            KanbanErrorCode::InvalidArgument,
                            "sprint create in --format json requires --headline (and other fields); interactive prompts are unavailable",
                        ),
                    });
                }
                let input = if *non_interactive || any_flag || mode == DispatchMode::Json {
                    let Some(headline) = headline else {
                        return Ok(CommandOutcome::Error {
                            kind: "sprint.create",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "--headline is required when creating a sprint non-interactively.",
                            ),
                        });
                    };
                    build_create_sprint_input_from_flags(
                        repo_root,
                        *number,
                        headline,
                        start.as_deref(),
                        end.as_deref(),
                    )?
                } else {
                    prompt_create_sprint(repo_root, None, None)?
                };
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::SprintCreate {
                    result: create_sprint(&root, &input)?,
                    repo_root: root,
                }
            }
            SprintCommand::Update {
                name,
                headline,
                start,
                end,
                status,
                wip_limit,
                repo_root,
            } => {
                if mode == DispatchMode::Human {
                    ensure_sprints_enabled(mode, repo_root)?;
                }
                let sprint = summarize_sprint(repo_root, name)?;
                let root = load_kanban_config(repo_root)?.repo_root;
                let sprint_path = root.join(&sprint.readme_path);
                let updates = if mode == DispatchMode::Json {
                    let updates = match json_story_frontmatter_updates(&[
                        ("headline", headline),
                        ("start_date", start),
                        ("end_date", end),
                        ("status", status),
                        ("wip_limit", wip_limit),
                    ]) {
                        Ok(updates) => updates,
                        Err(error) => {
                            return Ok(CommandOutcome::Error {
                                kind: "sprint.update",
                                body: KanbanErrorBody::new(
                                    KanbanErrorCode::InvalidArgument,
                                    error.to_string(),
                                ),
                            });
                        }
                    };
                    if updates.is_empty() {
                        return Ok(CommandOutcome::Error {
                            kind: "sprint.update",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "sprint update in --format json requires at least one frontmatter field; editor mode is unavailable.",
                            ),
                        });
                    }
                    updates
                } else {
                    let markdown = std::fs::read_to_string(&sprint_path)?;
                    let parsed = kanban_core::parse_frontmatter(&markdown);
                    let mut updates = Vec::new();
                    for (field_name, option) in [
                        ("headline", headline),
                        ("start_date", start),
                        ("end_date", end),
                        ("status", status),
                        ("wip_limit", wip_limit),
                    ] {
                        if let Some(update) =
                            sprint_frontmatter_update_value(&parsed, field_name, option)?
                        {
                            updates.push(update);
                        }
                    }
                    if updates.is_empty() {
                        open_markdown_in_editor(&sprint_path, "sprint markdown")?;
                        return Ok(CommandOutcome::Edited {
                            kind: "sprint.update",
                            path: sprint.readme_path,
                        });
                    }
                    updates
                };
                CommandOutcome::SprintUpdate {
                    result: update_sprint_frontmatter(&root, name, &updates)?,
                    repo_root: root,
                }
            }
            SprintCommand::Rollover { name, repo_root } => {
                if mode == DispatchMode::Human {
                    ensure_sprints_enabled(mode, repo_root)?;
                }
                let next_input = if mode == DispatchMode::Json {
                    None
                } else {
                    let sprint = summarize_sprint(repo_root, name)?;
                    let current_end = NaiveDate::parse_from_str(&sprint.end_date, "%Y-%m-%d")?;
                    let (suggested_start, suggested_end) = suggested_sprint_dates(current_end);
                    if summarize_sprints(repo_root)?.iter().any(|candidate| {
                        kanban_core::suggested_next_sprint_number(repo_root)
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
                            repo_root,
                            Some(suggested_start),
                            Some(suggested_end),
                        )?)
                    }
                };
                CommandOutcome::SprintRollover(rollover_sprint(
                    repo_root,
                    name,
                    next_input.as_ref(),
                )?)
            }
            SprintCommand::Sync { repo_root } => {
                if mode == DispatchMode::Human {
                    ensure_sprints_enabled(mode, repo_root)?;
                }
                CommandOutcome::SprintSync(sync_sprint_rosters(repo_root)?)
            }
        },
        Command::Phase {
            command: PhaseCommand::Show { phase, repo_root },
        } => {
            ensure_phases_enabled(mode, repo_root)?;
            CommandOutcome::PhaseShow(summarize_phase(repo_root, phase)?)
        }
        Command::Epic { command } => match command {
            EpicCommand::Show { id, repo_root } => {
                ensure_epics_enabled(mode, repo_root)?;
                CommandOutcome::EpicShow {
                    id: id.clone(),
                    result: Box::new(find_epic_with_source(repo_root, id)?),
                }
            }
            EpicCommand::Update {
                id,
                priority,
                planned_start,
                planned_end,
                work_started,
                work_done,
                repo_root,
            } => {
                ensure_epics_enabled(mode, repo_root)?;
                let updates = if mode == DispatchMode::Json {
                    let updates = match json_story_frontmatter_updates(&[
                        ("priority", priority),
                        ("planned_start", planned_start),
                        ("planned_end", planned_end),
                        ("work_started", work_started),
                        ("work_done", work_done),
                    ]) {
                        Ok(updates) => updates,
                        Err(error) => {
                            return Ok(CommandOutcome::Error {
                                kind: "epic.update",
                                body: KanbanErrorBody::new(
                                    KanbanErrorCode::InvalidArgument,
                                    error.to_string(),
                                ),
                            });
                        }
                    };
                    if updates.is_empty() {
                        return Ok(CommandOutcome::Error {
                            kind: "epic.update",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "epic update in --format json requires at least one frontmatter field; editor mode is unavailable.",
                            ),
                        });
                    }
                    updates
                } else {
                    let epic = find_epic_with_source(repo_root, id)?
                        .ok_or_else(|| {
                            anyhow::anyhow!("Epic not found: {}", id.trim().to_ascii_uppercase())
                        })?
                        .1;
                    let mut updates = Vec::new();
                    for (field_name, option) in [
                        ("priority", priority),
                        ("planned_start", planned_start),
                        ("planned_end", planned_end),
                        ("work_started", work_started),
                        ("work_done", work_done),
                    ] {
                        match option {
                            Some(Some(value)) => {
                                updates.push((field_name.to_string(), value.clone()))
                            }
                            Some(None) => {
                                let default = epic
                                    .frontmatter
                                    .get(field_name)
                                    .cloned()
                                    .unwrap_or_default();
                                updates.push((
                                    field_name.to_string(),
                                    prompt_with_default(field_name, &default)?,
                                ));
                            }
                            None => {}
                        }
                    }
                    if updates.is_empty() {
                        open_markdown_in_editor(&epic.file_path, "epic markdown")?;
                        return Ok(CommandOutcome::Edited {
                            kind: "epic.update",
                            path: epic.relative_path,
                        });
                    }
                    updates
                };
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::EpicUpdate {
                    result: update_epic_frontmatter(&root, id, &updates)?,
                    repo_root: root,
                }
            }
        },
        Command::Story { command } => match command {
            StoryCommand::Create {
                title,
                epic,
                id,
                status,
                sprint,
                story_points,
                assignee,
                priority,
                task_file,
                activated,
                work_started,
                work_done,
                created,
                updated,
                repo_root,
            } => {
                let input = CreateStoryInput {
                    id: id.clone(),
                    title: title.clone(),
                    epic_id: epic.clone(),
                    status: status.clone(),
                    sprint: sprint.clone(),
                    story_points: story_points.clone(),
                    assignee: assignee.clone(),
                    priority: priority.clone(),
                    task_file: task_file.clone(),
                    activated: activated.clone(),
                    work_started: work_started.clone(),
                    work_done: work_done.clone(),
                    created: created.clone(),
                    updated: updated.clone(),
                };
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::StoryCreate {
                    result: create_story(&root, &input)?,
                    repo_root: root,
                }
            }
            StoryCommand::Show { id, repo_root } => CommandOutcome::StoryShow {
                id: id.clone(),
                result: Box::new(find_story_with_source(repo_root, id)?),
            },
            StoryCommand::List {
                current,
                all,
                next,
                sprint,
                repo_root,
            } => resolve_story_list_scope(repo_root, *all, *next, *current, sprint.as_deref())
                .map(|(scope, stories)| CommandOutcome::StoryList { scope, stories })?,
            StoryCommand::Move {
                id,
                status,
                assignee,
                repo_root,
            } => {
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::StoryMove {
                    result: move_story_to_status_with_assignee(
                        &root,
                        id,
                        status,
                        assignee.as_deref(),
                    )?,
                    repo_root: root,
                }
            }
            StoryCommand::Plan {
                id,
                sprint,
                repo_root,
            } => {
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::StoryPlan {
                    result: plan_story_into_sprint(&root, id, sprint)?,
                    repo_root: root,
                }
            }
            StoryCommand::Delete { id, repo_root } => {
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::StoryDelete {
                    result: delete_story(&root, id)?,
                    repo_root: root,
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
                let updates = if mode == DispatchMode::Json {
                    let updates = match json_story_frontmatter_updates(&[
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
                    ]) {
                        Ok(updates) => updates,
                        Err(error) => {
                            return Ok(CommandOutcome::Error {
                                kind: "story.update",
                                body: KanbanErrorBody::new(
                                    KanbanErrorCode::InvalidArgument,
                                    error.to_string(),
                                ),
                            });
                        }
                    };
                    if updates.is_empty() {
                        return Ok(CommandOutcome::Error {
                            kind: "story.update",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "story update in --format json requires at least one frontmatter field; editor mode is unavailable.",
                            ),
                        });
                    }
                    updates
                } else {
                    let story_file = story_markdown_file(repo_root, id)?;
                    let story = read_story_file(&story_file.absolute_path, repo_root)?;
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
                            story_frontmatter_update_value(&story, field_name, option)?
                        {
                            updates.push(update);
                        }
                    }
                    if updates.is_empty() {
                        open_story_markdown_in_editor(&story_file.absolute_path)?;
                        return Ok(CommandOutcome::Edited {
                            kind: "story.update",
                            path: story_file.story_path,
                        });
                    }
                    updates
                };
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::StoryUpdate {
                    result: update_story_frontmatter(&root, id, &updates)?,
                    repo_root: root,
                }
            }
        },
        Command::Task { command } => match command {
            TaskCommand::Show {
                story_id,
                repo_root,
            } => {
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::TaskShow {
                    details: list_tasks_for_story(&root, story_id)?
                        .ok_or_else(|| anyhow::anyhow!("Story not found: {story_id}"))?,
                    repo_root: root,
                }
            }
            TaskCommand::Add {
                story_id,
                title,
                status,
                tags,
                description,
                repo_root,
            } => {
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::TaskMutation {
                    kind: "task.add",
                    result: add_task_to_story(&root, story_id, title, status, tags, description)?,
                    repo_root: root,
                }
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
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::TaskMutation {
                    kind: "task.update",
                    result: update_task_in_story(
                        &root,
                        story_id,
                        task_id,
                        status.as_deref(),
                        title.as_deref(),
                        tags.as_deref(),
                        description.as_deref(),
                    )?,
                    repo_root: root,
                }
            }
            TaskCommand::Delete {
                story_id,
                task_id,
                repo_root,
            } => {
                let root = load_kanban_config(repo_root)?.repo_root;
                CommandOutcome::TaskMutation {
                    kind: "task.delete",
                    result: delete_task_from_story(&root, story_id, task_id)?,
                    repo_root: root,
                }
            }
        },
        Command::Validate { repo_root } => {
            CommandOutcome::Validate(validate_repository(repo_root)?)
        }
        Command::Doctor {
            command: DoctorCommand::Show { repo_root },
        } => CommandOutcome::DoctorShow(doctor_repository(repo_root)?),
        Command::Doctor {
            command:
                DoctorCommand::Fix {
                    target,
                    non_interactive,
                    repo_root,
                },
        } => {
            if mode == DispatchMode::Json {
                return Ok(CommandOutcome::Error {
                    kind: "doctor.fix",
                    body: KanbanErrorBody::new(
                        KanbanErrorCode::InvalidArgument,
                        "doctor fix is not available in --format json mode; use `doctor show` instead.",
                    ),
                });
            }
            if *non_interactive {
                run_doctor_fix_non_interactive(theme, repo_root, target.as_deref())?;
            } else {
                run_doctor_fix_wizard(theme, repo_root, target.as_deref())?;
            }
            CommandOutcome::NoData { kind: "doctor.fix" }
        }
        Command::Report { command } => match command {
            ReportCommand::Wbs { repo_root } => {
                let stories = list_all_stories(repo_root)?;
                let sprints = report_sprints(repo_root)?;
                let epics = list_all_epic_overviews(repo_root)?;
                let current = report_current_sprint_name(repo_root, &sprints);
                let calendar = WorkingCalendar::load(&load_kanban_config(repo_root)?)?;
                CommandOutcome::ReportWbs(Box::new(ReportWbsDto::build_with_epics_and_calendar(
                    &stories,
                    &sprints,
                    &epics,
                    current.as_deref(),
                    calendar,
                )))
            }
            ReportCommand::Forecast { repo_root } => {
                let stories = list_all_stories(repo_root)?;
                let sprints = report_sprints(repo_root)?;
                let current = report_current_sprint_name(repo_root, &sprints);
                let calendar = WorkingCalendar::load(&load_kanban_config(repo_root)?)?;
                CommandOutcome::ReportForecast(ReportForecastDto::build_with_calendar(
                    &stories,
                    &sprints,
                    current.as_deref(),
                    calendar,
                ))
            }
        },
        Command::Web { command } => match command {
            WebCommand::Serve {
                repo_root,
                host,
                port,
            } => {
                if mode == DispatchMode::Json {
                    return Ok(CommandOutcome::Error {
                        kind: "web.serve",
                        body: KanbanErrorBody::new(
                            KanbanErrorCode::InvalidArgument,
                            "web serve is an internal server command and is not available in --format json mode.",
                        ),
                    });
                }
                kanban_web_server::serve_blocking(kanban_web_server::WebServeOptions {
                    repo_root: repo_root.clone(),
                    host: host.clone(),
                    port: *port,
                })?;
                CommandOutcome::NoData { kind: "web.serve" }
            }
            WebCommand::Status { repo_root } => {
                if mode == DispatchMode::Json {
                    CommandOutcome::WebStatus(web_status_json(repo_root)?)
                } else {
                    print_web_status(theme, repo_root)?;
                    CommandOutcome::NoData { kind: "web.status" }
                }
            }
            WebCommand::Start {
                foreground,
                open,
                dev,
                build,
                repo_root,
            } => {
                if mode == DispatchMode::Json {
                    if *foreground {
                        return Ok(CommandOutcome::Error {
                            kind: "web.start",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "web start --foreground is not available in --format json mode because it streams server output.",
                            ),
                        });
                    }
                    if *build {
                        return Ok(CommandOutcome::Error {
                            kind: "web.start",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "web start --build is not available in --format json mode because build output may not be JSON.",
                            ),
                        });
                    }
                    CommandOutcome::WebStart(web_start_json(repo_root, *open, *dev)?)
                } else {
                    start_web(theme, repo_root, *foreground, *open, *dev, *build)?;
                    CommandOutcome::NoData { kind: "web.start" }
                }
            }
            WebCommand::Stop { repo_root } => {
                if mode == DispatchMode::Json {
                    CommandOutcome::WebStop(web_stop_json(repo_root)?)
                } else {
                    stop_web(theme, repo_root, false)?;
                    CommandOutcome::NoData { kind: "web.stop" }
                }
            }
            WebCommand::Restart {
                open,
                dev,
                build,
                repo_root,
            } => {
                if mode == DispatchMode::Json {
                    if *build {
                        return Ok(CommandOutcome::Error {
                            kind: "web.restart",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "web restart --build is not available in --format json mode because build output may not be JSON.",
                            ),
                        });
                    }
                    let stopped_existing = stop_web(theme, repo_root, true)?;
                    CommandOutcome::WebRestart(WebRestartDto {
                        stopped_existing,
                        started: web_start_json(repo_root, *open, *dev)?,
                    })
                } else {
                    stop_web(theme, repo_root, true)?;
                    start_web(theme, repo_root, false, *open, *dev, *build)?;
                    CommandOutcome::NoData {
                        kind: "web.restart",
                    }
                }
            }
            WebCommand::Log {
                lines,
                follow,
                repo_root,
            } => {
                if mode == DispatchMode::Json {
                    if *follow {
                        return Ok(CommandOutcome::Error {
                            kind: "web.log",
                            body: KanbanErrorBody::new(
                                KanbanErrorCode::InvalidArgument,
                                "web log --follow is not available in --format json mode because it streams output.",
                            ),
                        });
                    }
                    CommandOutcome::WebLog(web_log_json(repo_root, *lines)?)
                } else {
                    print_web_log(theme, repo_root, *lines, *follow)?;
                    CommandOutcome::NoData { kind: "web.log" }
                }
            }
        },
        Command::Uninstall {
            prefix,
            skills_dir,
            yes,
            dry_run,
            quiet,
        } => {
            if mode == DispatchMode::Json {
                return Ok(CommandOutcome::Error {
                    kind: "uninstall",
                    body: KanbanErrorBody::new(
                        KanbanErrorCode::InvalidArgument,
                        "uninstall is not available in --format json mode because it runs an interactive system uninstaller.",
                    ),
                });
            }
            run_uninstall(UninstallOptions {
                prefix: prefix.clone(),
                skills_dir: skills_dir.clone(),
                yes: *yes,
                dry_run: *dry_run,
                quiet: *quiet,
            })?;
            CommandOutcome::NoData { kind: "uninstall" }
        }
        Command::Upgrade {
            prefix,
            skills_dir,
            no_skills,
            yes,
            force,
            dry_run,
            quiet,
        } => {
            if mode == DispatchMode::Json {
                return Ok(CommandOutcome::Error {
                    kind: "upgrade",
                    body: KanbanErrorBody::new(
                        KanbanErrorCode::InvalidArgument,
                        "upgrade is not available in --format json mode because it downloads and runs the remote installer.",
                    ),
                });
            }
            run_upgrade(UpgradeOptions {
                prefix: prefix.clone(),
                skills_dir: skills_dir.clone(),
                no_skills: *no_skills,
                yes: *yes,
                force: *force,
                dry_run: *dry_run,
                quiet: *quiet,
            })?;
            CommandOutcome::NoData { kind: "upgrade" }
        }
    })
}

pub(crate) fn completion_output(target: CompletionTarget) -> CompletionDto {
    let mut command = Args::command();
    if let Some(generator) = target.generator() {
        let mut buf = Vec::new();
        clap_complete::generate(generator, &mut command, "kanban", &mut buf);
        let script = String::from_utf8_lossy(&buf).into_owned();
        let content = match generator {
            clap_complete::Shell::Zsh => enhance_zsh_completion(&script),
            clap_complete::Shell::Bash => enhance_bash_completion(&script),
            _ => script,
        };
        CompletionDto {
            target: completion_target_label(target).to_string(),
            content_type: "shell-script".to_string(),
            content,
        }
    } else {
        CompletionDto {
            target: completion_target_label(target).to_string(),
            content_type: "help".to_string(),
            content: COMPLETION_HELP.to_string(),
        }
    }
}

pub(crate) fn list_id_items(
    kind: ListIdsKind,
    repo_root: &std::path::Path,
) -> Result<Vec<ListIdItemDto>> {
    match kind {
        ListIdsKind::Sprints => Ok(list_sprint_names(repo_root)?
            .into_iter()
            .map(ListIdItemDto::value)
            .collect()),
        ListIdsKind::Stories => Ok(list_story_ids(repo_root)?
            .into_iter()
            .map(ListIdItemDto::value)
            .collect()),
        ListIdsKind::StoriesWithTitles => Ok(list_story_completion_items(repo_root)?
            .iter()
            .map(ListIdItemDto::from_completion_item)
            .collect()),
        ListIdsKind::Epics => Ok(list_epic_ids(repo_root)?
            .into_iter()
            .map(ListIdItemDto::value)
            .collect()),
    }
}

pub(crate) fn feature_key(feature: FeatureName) -> &'static str {
    match feature {
        FeatureName::Sprints => "features.sprints",
        FeatureName::Epics => "features.epics",
        FeatureName::Phases => "features.phases",
    }
}

pub(crate) fn config_show_json_value(raw: &str) -> Result<serde_json::Value> {
    config_show_value(raw).map_err(|error| anyhow::anyhow!(error))
}
