use anyhow::Result;
use clap::CommandFactory;

use crate::cli::{
    Args, COMPLETION_HELP, Command, CompletionTarget, ConfigCommand, DoctorCommand, EpicCommand,
    FeatureName, FeaturesCommand, ListIdsKind, PhaseCommand, ReportCommand, SprintCommand,
    StoryCommand, TaskCommand, completion_target_label, list_ids_kind_label,
};
use crate::completion::{enhance_bash_completion, enhance_zsh_completion};
use crate::json_out::{
    ensure_epics_enabled_json, ensure_phases_enabled_json, ensure_sprints_enabled_json,
};
use crate::ops::resolve_story_list_scope;
use crate::outcome::{CommandOutcome, FeatureToggleAction};
use kanban_core::{
    CompletionDto, FeaturesConfig, ListIdItemDto, ReportForecastDto, ReportWbsDto,
    add_task_to_story, config_show_value, delete_story, delete_task_from_story, doctor_repository,
    find_epic_with_source, find_story, find_story_with_source, get_config_json, get_config_value,
    init_config_with_features, list_all_stories, list_epic_ids, list_sprint_names,
    list_story_completion_items, list_story_ids, list_tasks_for_story, load_kanban_config,
    move_story_to_status_with_assignee, plan_story_into_sprint, set_config_value,
    summarize_current_sprint, summarize_phase, summarize_sprint, summarize_sprints,
    sync_sprint_rosters, update_task_in_story, validate_repository,
};

pub(crate) fn execute_shared(command: &Command) -> Option<Result<CommandOutcome>> {
    Some(match command {
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
            init_config_with_features(repo_root, features).map(CommandOutcome::Init)
        }
        Command::Config { command } => match command {
            ConfigCommand::Show { repo_root } => {
                get_config_json(repo_root).map(CommandOutcome::ConfigShow)
            }
            ConfigCommand::Get { key, repo_root } => {
                get_config_value(repo_root, key).map(|value| CommandOutcome::ConfigGet {
                    key: key.clone(),
                    value,
                })
            }
            ConfigCommand::Set {
                key,
                value,
                repo_root,
            } => set_config_value(repo_root, key, value).map(CommandOutcome::ConfigSet),
        },
        Command::Features { command } => match command {
            FeaturesCommand::List { repo_root } => load_kanban_config(repo_root).map(|config| {
                let features = config.features();
                CommandOutcome::FeaturesList {
                    phases: features.phases,
                    sprints: features.sprints,
                    epics: features.epics,
                }
            }),
            FeaturesCommand::Enable { feature, repo_root } => {
                let key = feature_key(*feature);
                set_config_value(repo_root, key, "true").map(|result| {
                    CommandOutcome::FeatureToggle {
                        action: FeatureToggleAction::Enable,
                        result,
                    }
                })
            }
            FeaturesCommand::Disable { feature, repo_root } => {
                let key = feature_key(*feature);
                set_config_value(repo_root, key, "false").map(|result| {
                    CommandOutcome::FeatureToggle {
                        action: FeatureToggleAction::Disable,
                        result,
                    }
                })
            }
        },
        Command::Completion { target } => {
            Ok(CommandOutcome::Completion(completion_output(*target)))
        }
        Command::ListIds { kind, repo_root } => {
            list_id_items(*kind, repo_root).map(|items| CommandOutcome::ListIds {
                kind: list_ids_kind_label(*kind),
                items,
            })
        }
        Command::ListTaskIds {
            story_id,
            repo_root,
        } => find_story(repo_root, story_id).map(|details| {
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
        }),
        Command::Sprint { command } => match command {
            SprintCommand::Current { repo_root } => ensure_sprints_enabled_json(repo_root)
                .and_then(|_| summarize_current_sprint(repo_root))
                .map(|sprint| CommandOutcome::SprintOverview {
                    kind: "sprint.current",
                    sprint,
                    short: false,
                }),
            SprintCommand::List { repo_root } => {
                ensure_sprints_enabled_json(repo_root).and_then(|_| {
                    let sprints = summarize_sprints(repo_root)?;
                    let current_name = summarize_current_sprint(repo_root)
                        .ok()
                        .map(|sprint| sprint.sprint_name);
                    Ok(CommandOutcome::SprintList {
                        sprints,
                        current_name,
                    })
                })
            }
            SprintCommand::Show {
                name,
                short,
                repo_root,
            } => ensure_sprints_enabled_json(repo_root).and_then(|_| {
                let sprint = match name {
                    Some(name) => summarize_sprint(repo_root, name)?,
                    None => summarize_current_sprint(repo_root)?,
                };
                Ok(CommandOutcome::SprintOverview {
                    kind: "sprint.show",
                    sprint,
                    short: *short,
                })
            }),
            SprintCommand::Sync { repo_root } => {
                sync_sprint_rosters(repo_root).map(CommandOutcome::SprintSync)
            }
            _ => return None,
        },
        Command::Phase {
            command: PhaseCommand::Show { phase, repo_root },
        } => ensure_phases_enabled_json(repo_root)
            .and_then(|_| summarize_phase(repo_root, phase))
            .map(CommandOutcome::PhaseShow),
        Command::Epic {
            command: EpicCommand::Show { id, repo_root },
        } => ensure_epics_enabled_json(repo_root).and_then(|_| {
            find_epic_with_source(repo_root, id).map(|result| CommandOutcome::EpicShow {
                id: id.clone(),
                result: Box::new(result),
            })
        }),
        Command::Story { command } => match command {
            StoryCommand::Show { id, repo_root } => {
                find_story_with_source(repo_root, id).map(|result| CommandOutcome::StoryShow {
                    id: id.clone(),
                    result: Box::new(result),
                })
            }
            StoryCommand::List {
                current,
                all,
                next,
                sprint,
                repo_root,
            } => resolve_story_list_scope(repo_root, *all, *next, *current, sprint.as_deref())
                .map(|(scope, stories)| CommandOutcome::StoryList { scope, stories }),
            StoryCommand::Move {
                id,
                status,
                assignee,
                repo_root,
            } => {
                let root = match load_kanban_config(repo_root) {
                    Ok(config) => config.repo_root,
                    Err(error) => return Some(Err(error)),
                };
                move_story_to_status_with_assignee(&root, id, status, assignee.as_deref()).map(
                    |result| CommandOutcome::StoryMove {
                        result,
                        repo_root: root,
                    },
                )
            }
            StoryCommand::Plan {
                id,
                sprint,
                repo_root,
            } => {
                let root = match load_kanban_config(repo_root) {
                    Ok(config) => config.repo_root,
                    Err(error) => return Some(Err(error)),
                };
                plan_story_into_sprint(&root, id, sprint).map(|result| CommandOutcome::StoryPlan {
                    result,
                    repo_root: root,
                })
            }
            StoryCommand::Delete { id, repo_root } => {
                let root = match load_kanban_config(repo_root) {
                    Ok(config) => config.repo_root,
                    Err(error) => return Some(Err(error)),
                };
                delete_story(&root, id).map(|result| CommandOutcome::StoryDelete {
                    result,
                    repo_root: root,
                })
            }
            _ => return None,
        },
        Command::Task {
            command:
                TaskCommand::Show {
                    story_id,
                    repo_root,
                },
        } => {
            let root = match load_kanban_config(repo_root) {
                Ok(config) => config.repo_root,
                Err(error) => return Some(Err(error)),
            };
            list_tasks_for_story(&root, story_id).and_then(|details| {
                details
                    .map(|details| CommandOutcome::TaskShow {
                        details,
                        repo_root: root,
                    })
                    .ok_or_else(|| anyhow::anyhow!("Story not found: {story_id}"))
            })
        }
        Command::Task { command } => match command {
            TaskCommand::Add {
                story_id,
                title,
                status,
                tags,
                description,
                repo_root,
            } => {
                let root = match load_kanban_config(repo_root) {
                    Ok(config) => config.repo_root,
                    Err(error) => return Some(Err(error)),
                };
                add_task_to_story(&root, story_id, title, status, tags, description).map(|result| {
                    CommandOutcome::TaskMutation {
                        kind: "task.add",
                        result,
                        repo_root: root,
                    }
                })
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
                let root = match load_kanban_config(repo_root) {
                    Ok(config) => config.repo_root,
                    Err(error) => return Some(Err(error)),
                };
                update_task_in_story(
                    &root,
                    story_id,
                    task_id,
                    status.as_deref(),
                    title.as_deref(),
                    tags.as_deref(),
                    description.as_deref(),
                )
                .map(|result| CommandOutcome::TaskMutation {
                    kind: "task.update",
                    result,
                    repo_root: root,
                })
            }
            TaskCommand::Delete {
                story_id,
                task_id,
                repo_root,
            } => {
                let root = match load_kanban_config(repo_root) {
                    Ok(config) => config.repo_root,
                    Err(error) => return Some(Err(error)),
                };
                delete_task_from_story(&root, story_id, task_id).map(|result| {
                    CommandOutcome::TaskMutation {
                        kind: "task.delete",
                        result,
                        repo_root: root,
                    }
                })
            }
            TaskCommand::Show { .. } => return None,
        },
        Command::Validate { repo_root } => {
            validate_repository(repo_root).map(CommandOutcome::Validate)
        }
        Command::Doctor {
            command: DoctorCommand::Show { repo_root },
        } => doctor_repository(repo_root).map(CommandOutcome::DoctorShow),
        Command::Report { command } => match command {
            ReportCommand::Wbs { repo_root } => {
                let stories = list_all_stories(repo_root);
                let sprints = summarize_sprints(repo_root);
                match (stories, sprints) {
                    (Ok(stories), Ok(sprints)) => {
                        let current = summarize_current_sprint(repo_root)
                            .ok()
                            .map(|s| s.sprint_name);
                        Ok(CommandOutcome::ReportWbs(ReportWbsDto::build(
                            &stories,
                            &sprints,
                            current.as_deref(),
                        )))
                    }
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            }
            ReportCommand::Forecast { repo_root } => {
                let stories = list_all_stories(repo_root);
                let sprints = summarize_sprints(repo_root);
                match (stories, sprints) {
                    (Ok(stories), Ok(sprints)) => {
                        let current = summarize_current_sprint(repo_root)
                            .ok()
                            .map(|s| s.sprint_name);
                        Ok(CommandOutcome::ReportForecast(ReportForecastDto::build(
                            &stories,
                            &sprints,
                            current.as_deref(),
                        )))
                    }
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            }
        },
        _ => return None,
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
