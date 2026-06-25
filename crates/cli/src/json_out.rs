#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, layout::*, ops::*, prompt::*, render::*, theme::*, web::*,
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

fn feature_disabled_error(feature: &str, repo_root: &Path) -> anyhow::Error {
    anyhow::anyhow!(
        "Feature '{feature}' is disabled in .kanban/settings.json. Run `kanban features enable {feature}` to re-enable it. (repo: {})",
        repo_root.display()
    )
}

pub(crate) fn ensure_sprints_enabled_json(repo_root: &Path) -> anyhow::Result<()> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().sprints {
        return Err(feature_disabled_error("sprints", repo_root));
    }
    Ok(())
}

pub(crate) fn ensure_epics_enabled_json(repo_root: &Path) -> anyhow::Result<()> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().epics {
        return Err(feature_disabled_error("epics", repo_root));
    }
    Ok(())
}

pub(crate) fn ensure_phases_enabled_json(repo_root: &Path) -> anyhow::Result<()> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().phases {
        return Err(feature_disabled_error("phases", repo_root));
    }
    Ok(())
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

pub(crate) fn command_json_kind(command: &Command) -> &'static str {
    match command {
        Command::Init { .. } => "init",
        Command::Config { command } => match command {
            ConfigCommand::Show { .. } => "config.show",
            ConfigCommand::Get { .. } => "config.get",
            ConfigCommand::Set { .. } => "config.set",
        },
        Command::Sprint { command } => match command {
            SprintCommand::Current { .. } => "sprint.current",
            SprintCommand::List { .. } => "sprint.list",
            SprintCommand::Show { .. } => "sprint.show",
            SprintCommand::Create { .. } => "sprint.create",
            SprintCommand::Rollover { .. } => "sprint.rollover",
            SprintCommand::Sync { .. } => "sprint.sync",
        },
        Command::Phase { command } => match command {
            PhaseCommand::Show { .. } => "phase.show",
        },
        Command::Epic { command } => match command {
            EpicCommand::Show { .. } => "epic.show",
            EpicCommand::Update { .. } => "epic.update",
        },
        Command::Story { command } => match command {
            StoryCommand::Show { .. } => "story.show",
            StoryCommand::List { .. } => "story.list",
            StoryCommand::Move { .. } => "story.move",
            StoryCommand::Plan { .. } => "story.plan",
            StoryCommand::Delete { .. } => "story.delete",
            StoryCommand::Update { .. } => "story.update",
        },
        Command::Task { command } => match command {
            TaskCommand::Show { .. } => "task.show",
            TaskCommand::Add { .. } => "task.add",
            TaskCommand::Update { .. } => "task.update",
            TaskCommand::Delete { .. } => "task.delete",
        },
        Command::Web { command } => match command {
            WebCommand::Start { .. } => "web.start",
            WebCommand::Serve { .. } => "web.serve",
            WebCommand::Stop { .. } => "web.stop",
            WebCommand::Restart { .. } => "web.restart",
            WebCommand::Status { .. } => "web.status",
            WebCommand::Log { .. } => "web.log",
        },
        Command::Completion { .. } => "completion",
        Command::Uninstall { .. } => "uninstall",
        Command::Upgrade { .. } => "upgrade",
        Command::Validate { .. } => "validate",
        Command::Doctor { command } => match command {
            DoctorCommand::Show { .. } => "doctor.show",
            DoctorCommand::Fix { .. } => "doctor.fix",
        },
        Command::Report { command } => match command {
            ReportCommand::Wbs { .. } => "report.wbs",
            ReportCommand::Forecast { .. } => "report.forecast",
        },
        Command::Features { command } => match command {
            FeaturesCommand::List { .. } => "features.list",
            FeaturesCommand::Enable { .. } => "features.enable",
            FeaturesCommand::Disable { .. } => "features.disable",
        },
        Command::ListIds { .. } => "list-ids",
        Command::ListTaskIds { .. } => "list-task-ids",
    }
}

pub(crate) fn emit_json_git_requirement_error(
    command: &Command,
    message: impl Into<String>,
) -> i32 {
    print_envelope(&JsonEnvelope::<serde_json::Value>::error(
        command_json_kind(command),
        KanbanErrorBody::new(KanbanErrorCode::InvalidArgument, message),
    ))
}

/// Dispatch the JSON output path for a supported command.
pub(crate) fn emit_json(command: &Command) -> i32 {
    match command {
        Command::Init {
            repo_root,
            no_sprints,
            no_epics,
            no_phases,
        } => {
            let features = if *no_sprints || *no_epics || *no_phases {
                Some(kanban_core::FeaturesConfig {
                    phases: !*no_phases,
                    sprints: !*no_sprints,
                    epics: !*no_epics,
                })
            } else {
                None
            };
            match init_config_with_features(repo_root, features) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "init",
                    ConfigInitDto::from_result(&result),
                )),
                Err(error) => print_envelope(&JsonEnvelope::<ConfigInitDto>::error(
                    "init",
                    KanbanErrorBody::from_anyhow(&error),
                )),
            }
        }
        Command::Features { command } => match command {
            FeaturesCommand::List { repo_root } => match load_kanban_config(repo_root) {
                Ok(config) => {
                    let features = config.features();
                    let env = JsonEnvelope::ok(
                        "features.list",
                        serde_json::json!({
                            "phases": features.phases,
                            "sprints": features.sprints,
                            "epics": features.epics,
                        }),
                    );
                    print_envelope(&env)
                }
                Err(error) => print_envelope(&JsonEnvelope::<serde_json::Value>::error(
                    "features.list",
                    KanbanErrorBody::from_anyhow(&error),
                )),
            },
            FeaturesCommand::Enable { feature, repo_root } => {
                let key = match feature {
                    FeatureName::Sprints => "features.sprints",
                    FeatureName::Epics => "features.epics",
                    FeatureName::Phases => "features.phases",
                };
                match set_config_value(repo_root, key, "true") {
                    Ok(result) => print_envelope(&JsonEnvelope::ok(
                        "features.enable",
                        ConfigSetDto::from_result(&result),
                    )),
                    Err(error) => print_envelope(&JsonEnvelope::<ConfigSetDto>::error(
                        "features.enable",
                        KanbanErrorBody::from_anyhow(&error),
                    )),
                }
            }
            FeaturesCommand::Disable { feature, repo_root } => {
                let key = match feature {
                    FeatureName::Sprints => "features.sprints",
                    FeatureName::Epics => "features.epics",
                    FeatureName::Phases => "features.phases",
                };
                match set_config_value(repo_root, key, "false") {
                    Ok(result) => print_envelope(&JsonEnvelope::ok(
                        "features.disable",
                        ConfigSetDto::from_result(&result),
                    )),
                    Err(error) => print_envelope(&JsonEnvelope::<ConfigSetDto>::error(
                        "features.disable",
                        KanbanErrorBody::from_anyhow(&error),
                    )),
                }
            }
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
        } => {
            if let Err(error) = ensure_epics_enabled_json(repo_root) {
                return print_envelope(&JsonEnvelope::<EpicShowDto>::error(
                    "epic.show",
                    KanbanErrorBody::from_anyhow(&error),
                ));
            }
            match find_epic_with_source(repo_root, id) {
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
            }
        }
        Command::Epic {
            command:
                EpicCommand::Update {
                    id,
                    priority,
                    repo_root,
                },
        } => {
            if let Err(error) = ensure_epics_enabled_json(repo_root) {
                return print_envelope(&JsonEnvelope::<EpicUpdateDto>::error(
                    "epic.update",
                    KanbanErrorBody::from_anyhow(&error),
                ));
            }
            let updates = match json_story_frontmatter_updates(&[("priority", priority)]) {
                Ok(updates) => updates,
                Err(error) => {
                    return print_envelope(&JsonEnvelope::<EpicUpdateDto>::error(
                        "epic.update",
                        KanbanErrorBody::new(KanbanErrorCode::InvalidArgument, error.to_string()),
                    ));
                }
            };
            if updates.is_empty() {
                return invalid_argument_envelope::<EpicUpdateDto>(
                    "epic.update",
                    "epic update in --format json requires at least one frontmatter field.",
                );
            }
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<EpicUpdateDto>::error(
                        "epic.update",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match update_epic_frontmatter(&root, id, &updates) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "epic.update",
                    EpicUpdateDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<EpicUpdateDto>::error(
                    "epic.update",
                    KanbanErrorBody::from_anyhow(&e),
                )),
            }
        }
        Command::Story {
            command:
                StoryCommand::List {
                    all,
                    next,
                    current,
                    sprint,
                    repo_root,
                },
        } => {
            // US-020: shared scope resolution — both human and JSON paths call
            // resolve_story_list_scope so behavior cannot drift.
            match resolve_story_list_scope(repo_root, *all, *next, *current, sprint.as_deref()) {
                Ok((scope, stories)) => {
                    let env = JsonEnvelope::ok(
                        "story.list",
                        StoryListDto::new(scope.json_label(), &stories),
                    );
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
        } => match ensure_sprints_enabled_json(repo_root)
            .and_then(|_| summarize_current_sprint(repo_root))
        {
            Ok(overview) => print_envelope(&JsonEnvelope::ok(
                "sprint.current",
                SprintOverviewDto::from_overview(&overview),
            )),
            Err(error) => print_envelope(&JsonEnvelope::<SprintOverviewDto>::error(
                "sprint.current",
                KanbanErrorBody::from_anyhow(&error),
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
            if let Err(error) = ensure_sprints_enabled_json(repo_root) {
                return print_envelope(&JsonEnvelope::<SprintOverviewDto>::error(
                    "sprint.show",
                    KanbanErrorBody::from_anyhow(&error),
                ));
            }
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
        } => {
            if let Err(error) = ensure_sprints_enabled_json(repo_root) {
                return print_envelope(&JsonEnvelope::<SprintListDto>::error(
                    "sprint.list",
                    KanbanErrorBody::from_anyhow(&error),
                ));
            }
            match summarize_sprints(repo_root) {
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
            }
        }
        Command::Phase {
            command: PhaseCommand::Show { phase, repo_root },
        } => {
            if let Err(error) = ensure_phases_enabled_json(repo_root) {
                return print_envelope(&JsonEnvelope::<PhaseShowDto>::error(
                    "phase.show",
                    KanbanErrorBody::from_anyhow(&error),
                ));
            }
            match summarize_phase(repo_root, phase) {
                Ok(overview) => print_envelope(&JsonEnvelope::ok(
                    "phase.show",
                    PhaseShowDto::from_overview(&overview),
                )),
                Err(error) => print_envelope(&JsonEnvelope::<PhaseShowDto>::error(
                    "phase.show",
                    KanbanErrorBody::new(KanbanErrorCode::PhaseNotFound, error.to_string()),
                )),
            }
        }
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
            command: StoryCommand::Delete { id, repo_root },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(c) => c.repo_root,
                Err(e) => {
                    return print_envelope(&JsonEnvelope::<DeleteStoryDto>::error(
                        "story.delete",
                        KanbanErrorBody::new(KanbanErrorCode::NotInitialized, e.to_string()),
                    ));
                }
            };
            match delete_story(&root, id) {
                Ok(result) => print_envelope(&JsonEnvelope::ok(
                    "story.delete",
                    DeleteStoryDto::from_result(&result, &root),
                )),
                Err(e) => print_envelope(&JsonEnvelope::<DeleteStoryDto>::error(
                    "story.delete",
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
                    priority,
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
            // US-020: shared input builder — both human and JSON paths call
            // build_create_sprint_input_from_flags so behavior cannot drift.
            let input_result = build_create_sprint_input_from_flags(
                &root,
                *number,
                headline_val,
                start.as_deref(),
                end.as_deref(),
            );
            match input_result.and_then(|input| create_sprint(&root, &input)) {
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
            WebCommand::Serve { .. } => print_envelope(&JsonEnvelope::<NoData>::error(
                "web.serve",
                KanbanErrorBody::new(
                    KanbanErrorCode::InvalidArgument,
                    "web serve is an internal server command and is not available in --format json mode.",
                ),
            )),
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
        Command::Uninstall { .. } => print_envelope(&JsonEnvelope::<NoData>::error(
            "uninstall",
            KanbanErrorBody::new(
                KanbanErrorCode::InvalidArgument,
                "uninstall is not available in --format json mode because it runs an interactive system uninstaller.",
            ),
        )),
        Command::Upgrade { .. } => print_envelope(&JsonEnvelope::<NoData>::error(
            "upgrade",
            KanbanErrorBody::new(
                KanbanErrorCode::InvalidArgument,
                "upgrade is not available in --format json mode because it downloads and runs the remote installer.",
            ),
        )),
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
