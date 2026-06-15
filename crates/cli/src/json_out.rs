#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, layout::*, prompt::*, render::*, theme::*, web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

/// Serialize a `JsonEnvelope` to stdout and return its exit code.
pub(crate) fn print_envelope<T: Serialize>(env: &JsonEnvelope<T>) -> i32 {
    match serde_json::to_string_pretty(env) {
        Ok(json) => {
            println!("{json}");
            env.exit_code()
        }
        Err(_) => {
            let fallback = r#"{"status":"error","kind":"unknown","schema_version":1,"data":null,"error":{"code":"internal","message":"JSON serialization failed","details":null}}"#;
            println!("{fallback}");
            1
        }
    }
}

pub(crate) fn invalid_argument_envelope<T: Serialize>(
    kind: &'static str,
    message: impl Into<String>,
) -> i32 {
    print_envelope(&JsonEnvelope::<T>::error(
        kind,
        KanbanErrorBody::new(KanbanErrorCode::InvalidArgument, message),
    ))
}

pub(crate) fn completion_output(target: CompletionTarget) -> CompletionDto {
    let mut command = Args::command();
    if let Some(generator) = target.generator() {
        let mut buf = Vec::new();
        clap_complete::generate(generator, &mut command, "kanban", &mut buf);
        let script = String::from_utf8(buf).expect("clap_complete output should be utf8");
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

pub(crate) fn json_story_frontmatter_updates(
    fields: &[(&str, &Option<Option<String>>)],
) -> Result<Vec<(String, String)>> {
    let mut updates = Vec::new();
    for (field_name, option) in fields {
        match option {
            None => {}
            Some(Some(value)) => updates.push(((*field_name).to_string(), value.clone())),
            Some(None) => bail!("--{field_name} requires a value in --format json mode."),
        }
    }
    Ok(updates)
}

pub(crate) fn forward_slashed_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Dispatch the JSON output path for a supported command.
pub(crate) fn emit_json(command: &Command) -> i32 {
    match command {
        Command::Init { repo_root } => match init_config(repo_root) {
            Ok(result) => print_envelope(&JsonEnvelope::ok(
                "init",
                ConfigInitDto::from_result(&result),
            )),
            Err(error) => print_envelope(&JsonEnvelope::<ConfigInitDto>::error(
                "init",
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        Command::Config {
            command: ConfigCommand::Get { key, repo_root },
        } => match get_config_value(repo_root, key) {
            Ok(value) => {
                let env = JsonEnvelope::ok(
                    "config.get",
                    ConfigGetDto {
                        key: key.clone(),
                        value,
                    },
                );
                print_envelope(&env)
            }
            Err(error) => {
                let env: JsonEnvelope<ConfigGetDto> = JsonEnvelope::error(
                    "config.get",
                    KanbanErrorBody::new(KanbanErrorCode::ConfigKeyNotFound, error.to_string()),
                );
                print_envelope(&env)
            }
        },
        Command::Story {
            command: StoryCommand::Show { id, repo_root },
        } => match find_story_with_source(repo_root, id) {
            Ok(Some((details, source))) => {
                let dto = StoryShowDto::from_details_and_source(&details, &source);
                print_envelope(&JsonEnvelope::ok("story.show", dto))
            }
            Ok(None) => {
                let body = KanbanErrorBody::new(
                    KanbanErrorCode::StoryNotFound,
                    format!("No story matches id '{id}'"),
                );
                print_envelope(&JsonEnvelope::<StoryShowDto>::error("story.show", body))
            }
            Err(error) => print_envelope(&JsonEnvelope::<StoryShowDto>::error(
                "story.show",
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        Command::Epic {
            command: EpicCommand::Show { id, repo_root },
        } => match find_epic_with_source(repo_root, id) {
            Ok(Some((details, source))) => {
                let dto = EpicShowDto::from_details_and_source(&details, &source);
                print_envelope(&JsonEnvelope::ok("epic.show", dto))
            }
            Ok(None) => {
                let body = KanbanErrorBody::new(
                    KanbanErrorCode::EpicNotFound,
                    format!("No epic matches id '{id}'"),
                );
                print_envelope(&JsonEnvelope::<EpicShowDto>::error("epic.show", body))
            }
            Err(error) => print_envelope(&JsonEnvelope::<EpicShowDto>::error(
                "epic.show",
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        Command::Story {
            command:
                StoryCommand::List {
                    all,
                    next,
                    sprint,
                    repo_root,
                    ..
                },
        } => {
            // Resolve scope label and story list; current/next return (name, stories) tuples.
            let list_result: Result<(String, Vec<StoryOverview>), _> = if *all {
                list_all_stories(repo_root).map(|stories| ("all".to_string(), stories))
            } else if *next {
                list_next_sprint_stories(repo_root)
                    .map(|(_name, stories)| ("next".to_string(), stories))
            } else if let Some(sprint_id) = sprint {
                list_stories_in_sprint(repo_root, sprint_id)
                    .map(|stories| (format!("sprint:{sprint_id}"), stories))
            } else {
                list_current_sprint_stories(repo_root)
                    .map(|(_name, stories)| ("current".to_string(), stories))
            };
            match list_result {
                Ok((scope, stories)) => {
                    let env = JsonEnvelope::ok("story.list", StoryListDto::new(scope, &stories));
                    print_envelope(&env)
                }
                Err(e) => {
                    let env: JsonEnvelope<StoryListDto> =
                        JsonEnvelope::error("story.list", KanbanErrorBody::from_anyhow(&e));
                    print_envelope(&env)
                }
            }
        }
        Command::Sprint {
            command: SprintCommand::Current { repo_root },
        } => match summarize_current_sprint(repo_root) {
            Ok(overview) => print_envelope(&JsonEnvelope::ok(
                "sprint.current",
                SprintOverviewDto::from_overview(&overview),
            )),
            Err(error) => print_envelope(&JsonEnvelope::<SprintOverviewDto>::error(
                "sprint.current",
                KanbanErrorBody::new(KanbanErrorCode::SprintNotFound, error.to_string()),
            )),
        },
        Command::Sprint {
            command:
                SprintCommand::Show {
                    name,
                    short: _short,
                    repo_root,
                },
        } => {
            let sprint_result = match name {
                Some(name) => summarize_sprint(repo_root, name),
                None => summarize_current_sprint(repo_root),
            };
            match sprint_result {
                Ok(overview) => print_envelope(&JsonEnvelope::ok(
                    "sprint.show",
                    SprintOverviewDto::from_overview(&overview),
                )),
                Err(error) => print_envelope(&JsonEnvelope::<SprintOverviewDto>::error(
                    "sprint.show",
                    KanbanErrorBody::new(KanbanErrorCode::SprintNotFound, error.to_string()),
                )),
            }
        }
        Command::Sprint {
            command: SprintCommand::List { repo_root },
        } => match summarize_sprints(repo_root) {
            Ok(sprints) => {
                let current = summarize_current_sprint(repo_root)
                    .ok()
                    .map(|c| c.sprint_name);
                let dto = SprintListDto::new(&sprints, current.as_deref());
                print_envelope(&JsonEnvelope::ok("sprint.list", dto))
            }
            Err(e) => print_envelope(&JsonEnvelope::<SprintListDto>::error(
                "sprint.list",
                KanbanErrorBody::from_anyhow(&e),
            )),
        },
        Command::Phase {
            command: PhaseCommand::Show { phase, repo_root },
        } => match summarize_phase(repo_root, phase) {
            Ok(overview) => print_envelope(&JsonEnvelope::ok(
                "phase.show",
                PhaseShowDto::from_overview(&overview),
            )),
            Err(error) => print_envelope(&JsonEnvelope::<PhaseShowDto>::error(
                "phase.show",
                KanbanErrorBody::new(KanbanErrorCode::PhaseNotFound, error.to_string()),
            )),
        },
        Command::Config {
            command: ConfigCommand::Show { repo_root },
        } => match get_config_json(repo_root)
            .and_then(|s| config_show_value(&s).map_err(|e| anyhow::anyhow!(e)))
        {
            Ok(value) => print_envelope(&JsonEnvelope::ok("config.show", value)),
            Err(error) => print_envelope(&JsonEnvelope::<serde_json::Value>::error(
                "config.show",
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        Command::Config {
            command:
                ConfigCommand::Set {
                    key,
                    value,
                    repo_root,
                },
        } => match set_config_value(repo_root, key, value) {
            Ok(result) => print_envelope(&JsonEnvelope::ok(
                "config.set",
                ConfigSetDto::from_result(&result),
            )),
            Err(error) => print_envelope(&JsonEnvelope::<ConfigSetDto>::error(
                "config.set",
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        Command::Validate { repo_root } => match validate_repository(repo_root) {
            Ok(report) => {
                let dto = ValidateDto::from_report(&report, &report.repo_root);
                let env = if dto.valid {
                    JsonEnvelope::ok("validate", dto)
                } else {
                    JsonEnvelope::warning("validate", dto)
                };
                print_envelope(&env)
            }
            Err(error) => print_envelope(&JsonEnvelope::<ValidateDto>::error(
                "validate",
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        Command::Doctor {
            command: DoctorCommand::Show { repo_root },
        } => match doctor_repository(repo_root) {
            Ok(findings) => {
                let dto = DoctorDto::from_findings(&findings);
                let env = if dto.healthy {
                    JsonEnvelope::ok("doctor", dto)
                } else {
                    JsonEnvelope::warning("doctor", dto)
                };
                print_envelope(&env)
            }
            Err(error) => print_envelope(&JsonEnvelope::<DoctorDto>::error(
                "doctor",
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        Command::Story {
            command:
                StoryCommand::Move {
                    id,
                    status,
                    assignee,
                    repo_root,
                },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<MoveStoryDto>::error(
                        "story.move",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match move_story_to_status_with_assignee(&root, id, status, assignee.as_deref()) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "story.move",
                    MoveStoryDto::from_result(&result, &root),
                )),
                Err(e) => {
                    let body = if e.to_string().to_lowercase().contains("status") {
                        KanbanErrorBody::new(KanbanErrorCode::InvalidStatus, e.to_string())
                    } else {
                        KanbanErrorBody::from_anyhow(&e)
                    };
                    print_envelope(&JsonEnvelope::<MoveStoryDto>::error("story.move", body))
                }
            }
        }
        Command::Story {
            command:
                StoryCommand::Plan {
                    id,
                    sprint,
                    repo_root,
                },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<PlanStoryDto>::error(
                        "story.plan",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match plan_story_into_sprint(&root, id, sprint) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "story.plan",
                    PlanStoryDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<PlanStoryDto>::error(
                    "story.plan",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Story {
            command:
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
                },
        } => {
            let updates = match json_story_frontmatter_updates(&[
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
            ]) {
                Ok(updates) => updates,
                Err(error) => {
                    return print_envelope(&JsonEnvelope::<StoryUpdateDto>::error(
                        "story.update",
                        KanbanErrorBody::new(KanbanErrorCode::InvalidArgument, error.to_string()),
                    ));
                }
            };
            if updates.is_empty() {
                return invalid_argument_envelope::<StoryUpdateDto>(
                    "story.update",
                    "story update in --format json requires at least one frontmatter field; editor mode is unavailable.",
                );
            }
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<StoryUpdateDto>::error(
                        "story.update",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match update_story_frontmatter(&root, id, &updates) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "story.update",
                    StoryUpdateDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<StoryUpdateDto>::error(
                    "story.update",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Task {
            command:
                TaskCommand::Show {
                    story_id,
                    repo_root,
                },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<TaskShowDto>::error(
                        "task.show",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match list_tasks_for_story(&root, story_id) {
                Ok(Some(result)) => print_envelope(&JsonEnvelope::ok(
                    "task.show",
                    TaskShowDto::from_result(&result, &root),
                )),
                Ok(None) => print_envelope(&JsonEnvelope::<TaskShowDto>::error(
                    "task.show",
                    KanbanErrorBody::new(
                        KanbanErrorCode::StoryNotFound,
                        format!("Story not found: {story_id}"),
                    ),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<TaskShowDto>::error(
                    "task.show",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Task {
            command:
                TaskCommand::Add {
                    story_id,
                    title,
                    status,
                    tags,
                    description,
                    repo_root,
                },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<TaskMutationDto>::error(
                        "task.add",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match add_task_to_story(&root, story_id, title, status, tags, description) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "task.add",
                    TaskMutationDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<TaskMutationDto>::error(
                    "task.add",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Task {
            command:
                TaskCommand::Update {
                    story_id,
                    task_id,
                    title,
                    status,
                    tags,
                    description,
                    repo_root,
                },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<TaskMutationDto>::error(
                        "task.update",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match update_task_in_story(
                &root,
                story_id,
                task_id,
                status.as_deref(),
                title.as_deref(),
                tags.as_deref(),
                description.as_deref(),
            ) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "task.update",
                    TaskMutationDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<TaskMutationDto>::error(
                    "task.update",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Task {
            command:
                TaskCommand::Delete {
                    story_id,
                    task_id,
                    repo_root,
                },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<TaskMutationDto>::error(
                        "task.delete",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match delete_task_from_story(&root, story_id, task_id) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "task.delete",
                    TaskMutationDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<TaskMutationDto>::error(
                    "task.delete",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Sprint {
            command:
                SprintCommand::Create {
                    number,
                    headline,
                    start,
                    end,
                    non_interactive,
                    repo_root,
                },
        } => {
            let any_flag =
                number.is_some() || headline.is_some() || start.is_some() || end.is_some();
            if !non_interactive && !any_flag {
                return print_envelope(&JsonEnvelope::<SprintCreateDto>::error(
                    "sprint.create",
                    KanbanErrorBody::new(
                        KanbanErrorCode::InvalidArgument,
                        "sprint create in --format json requires --headline (and other fields); interactive prompts are unavailable",
                    ),
                ));
            }
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<SprintCreateDto>::error(
                        "sprint.create",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            let headline_val = match headline {
                Some(h) => h,
                None => {
                    return print_envelope(&JsonEnvelope::<SprintCreateDto>::error(
                        "sprint.create",
                        KanbanErrorBody::new(
                            KanbanErrorCode::InvalidArgument,
                            "--headline is required when creating a sprint non-interactively.",
                        ),
                    ));
                }
            };
            let build_input = || -> anyhow::Result<CreateSprintInput> {
                let number_val = match number {
                    Some(v) => *v,
                    None => suggested_next_sprint_number(&root)?,
                };
                let repo_suggestion = suggested_next_sprint_dates(&root)?;
                let today = chrono::Local::now().date_naive();
                let start_date = match start {
                    Some(v) => NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d")
                        .map_err(|_| anyhow::anyhow!("--start must be a date as YYYY-MM-DD."))?,
                    None => repo_suggestion.map(|(s, _)| s).unwrap_or(today),
                };
                let end_date = match end {
                    Some(v) => NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d")
                        .map_err(|_| anyhow::anyhow!("--end must be a date as YYYY-MM-DD."))?,
                    None => repo_suggestion
                        .map(|(_, e)| e)
                        .unwrap_or_else(|| suggested_sprint_dates(start_date).1),
                };
                Ok(CreateSprintInput {
                    number: number_val,
                    start_date,
                    end_date,
                    headline: headline_val.clone(),
                })
            };
            match build_input().and_then(|input| create_sprint(&root, &input)) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "sprint.create",
                    SprintCreateDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<SprintCreateDto>::error(
                    "sprint.create",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Sprint {
            command: SprintCommand::Rollover { name, repo_root },
        } => {
            // In JSON mode, rollover only succeeds when next sprint already exists;
            // we do not prompt for next sprint details.
            match rollover_sprint(repo_root, name, None) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "sprint.rollover",
                    SprintRolloverDto::from_result(&result),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<SprintRolloverDto>::error(
                    "sprint.rollover",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Sprint {
            command: SprintCommand::Sync { repo_root },
        } => match sync_sprint_rosters(repo_root) {
            Ok(changed) => print_envelope(&JsonEnvelope::ok(
                "sprint.sync",
                SprintSyncDto::from_changed(changed),
            )),
            Err(e) => print_envelope(&JsonEnvelope::<SprintSyncDto>::error(
                "sprint.sync",
                KanbanErrorBody::from_anyhow(&e),
            )),
        },
        Command::Web { command } => match command {
            WebCommand::Status { repo_root } => match web_status_json(repo_root) {
                Ok(status) => print_envelope(&JsonEnvelope::ok("web.status", status)),
                Err(error) => print_envelope(&JsonEnvelope::<WebStatusDto>::error(
                    "web.status",
                    KanbanErrorBody::from_anyhow(&error),
                )),
            },
            WebCommand::Start {
                foreground,
                open,
                dev,
                build,
                repo_root,
            } => {
                if *foreground {
                    return invalid_argument_envelope::<WebStartDto>(
                        "web.start",
                        "web start --foreground is not available in --format json mode because it streams server output.",
                    );
                }
                if *build {
                    return invalid_argument_envelope::<WebStartDto>(
                        "web.start",
                        "web start --build is not available in --format json mode because build output may not be JSON.",
                    );
                }
                match web_start_json(repo_root, *open, *dev) {
                    Ok(started) => print_envelope(&JsonEnvelope::ok("web.start", started)),
                    Err(error) => print_envelope(&JsonEnvelope::<WebStartDto>::error(
                        "web.start",
                        KanbanErrorBody::from_anyhow(&error),
                    )),
                }
            }
            WebCommand::Stop { repo_root } => match web_stop_json(repo_root) {
                Ok(stopped) => print_envelope(&JsonEnvelope::ok("web.stop", stopped)),
                Err(error) => print_envelope(&JsonEnvelope::<WebStopDto>::error(
                    "web.stop",
                    KanbanErrorBody::from_anyhow(&error),
                )),
            },
            WebCommand::Restart {
                open,
                dev,
                build,
                repo_root,
            } => {
                if *build {
                    return invalid_argument_envelope::<WebRestartDto>(
                        "web.restart",
                        "web restart --build is not available in --format json mode because build output may not be JSON.",
                    );
                }
                let stopped_existing =
                    match stop_web(&Theme::for_stdout(ColorMode::Never), repo_root, true) {
                        Ok(stopped) => stopped,
                        Err(error) => {
                            return print_envelope(&JsonEnvelope::<WebRestartDto>::error(
                                "web.restart",
                                KanbanErrorBody::from_anyhow(&error),
                            ));
                        }
                    };
                match web_start_json(repo_root, *open, *dev) {
                    Ok(started) => print_envelope(&JsonEnvelope::ok(
                        "web.restart",
                        WebRestartDto {
                            stopped_existing,
                            started,
                        },
                    )),
                    Err(error) => print_envelope(&JsonEnvelope::<WebRestartDto>::error(
                        "web.restart",
                        KanbanErrorBody::from_anyhow(&error),
                    )),
                }
            }
            WebCommand::Log {
                lines,
                follow,
                repo_root,
            } => {
                if *follow {
                    return invalid_argument_envelope::<WebLogDto>(
                        "web.log",
                        "web log --follow is not available in --format json mode because it streams output.",
                    );
                }
                match web_log_json(repo_root, *lines) {
                    Ok(log) => print_envelope(&JsonEnvelope::ok("web.log", log)),
                    Err(error) => print_envelope(&JsonEnvelope::<WebLogDto>::error(
                        "web.log",
                        KanbanErrorBody::from_anyhow(&error),
                    )),
                }
            }
        },
        Command::Doctor {
            command: DoctorCommand::Fix { .. },
        } => print_envelope(&JsonEnvelope::<NoData>::error(
            "doctor.fix",
            KanbanErrorBody::new(
                KanbanErrorCode::InvalidArgument,
                "doctor fix is not available in --format json mode; use `doctor show` instead.",
            ),
        )),
        Command::Completion { target } => {
            print_envelope(&JsonEnvelope::ok("completion", completion_output(*target)))
        }
        Command::Report { command } => {
            let repo_root = match command {
                ReportCommand::Wbs { repo_root } | ReportCommand::Forecast { repo_root } => {
                    repo_root
                }
            };
            let stories_result = list_all_stories(repo_root);
            let sprints_result = summarize_sprints(repo_root);
            let current = summarize_current_sprint(repo_root)
                .ok()
                .map(|s| s.sprint_name);
            match (stories_result, sprints_result, command) {
                (Ok(stories), Ok(sprints), ReportCommand::Wbs { .. }) => {
                    let dto = ReportWbsDto::build(&stories, &sprints, current.as_deref());
                    print_envelope(&JsonEnvelope::ok("report.wbs", dto))
                }
                (Ok(stories), Ok(sprints), ReportCommand::Forecast { .. }) => {
                    let dto = ReportForecastDto::build(&stories, &sprints, current.as_deref());
                    print_envelope(&JsonEnvelope::ok("report.forecast", dto))
                }
                (Err(e), _, ReportCommand::Wbs { .. }) | (_, Err(e), ReportCommand::Wbs { .. }) => {
                    print_envelope(&JsonEnvelope::<ReportWbsDto>::error(
                        "report.wbs",
                        KanbanErrorBody::from_anyhow(&e),
                    ))
                }
                (Err(e), _, ReportCommand::Forecast { .. })
                | (_, Err(e), ReportCommand::Forecast { .. }) => {
                    print_envelope(&JsonEnvelope::<ReportForecastDto>::error(
                        "report.forecast",
                        KanbanErrorBody::from_anyhow(&e),
                    ))
                }
            }
        }
        Command::ListIds { kind, repo_root } => {
            let kind_label = list_ids_kind_label(*kind);
            let items_result = match kind {
                ListIdsKind::Sprints => list_sprint_names(repo_root)
                    .map(|items| items.into_iter().map(ListIdItemDto::value).collect()),
                ListIdsKind::Stories => list_story_ids(repo_root)
                    .map(|items| items.into_iter().map(ListIdItemDto::value).collect()),
                ListIdsKind::StoriesWithTitles => {
                    list_story_completion_items(repo_root).map(|items| {
                        items
                            .iter()
                            .map(ListIdItemDto::from_completion_item)
                            .collect()
                    })
                }
                ListIdsKind::Epics => list_epic_ids(repo_root)
                    .map(|items| items.into_iter().map(ListIdItemDto::value).collect()),
            };
            match items_result {
                Ok(items) => print_envelope(&JsonEnvelope::ok(
                    "list-ids",
                    ListIdsDto::new(kind_label, items),
                )),
                Err(error) => print_envelope(&JsonEnvelope::<ListIdsDto>::error(
                    "list-ids",
                    KanbanErrorBody::from_anyhow(&error),
                )),
            }
        }
        Command::ListTaskIds {
            story_id,
            repo_root,
        } => {
            let items_result = find_story(repo_root, story_id).map(|details| {
                details
                    .map(|details| {
                        details
                            .tasks
                            .into_iter()
                            .map(|task| ListIdItemDto::value(task.id))
                            .collect()
                    })
                    .unwrap_or_default()
            });
            match items_result {
                Ok(items) => print_envelope(&JsonEnvelope::ok(
                    "list-task-ids",
                    ListIdsDto::new("tasks", items),
                )),
                Err(error) => print_envelope(&JsonEnvelope::<ListIdsDto>::error(
                    "list-task-ids",
                    KanbanErrorBody::from_anyhow(&error),
                )),
            }
        }
    }
}
