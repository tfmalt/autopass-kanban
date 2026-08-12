use anyhow::{Result, bail};
use serde::Serialize;
use std::path::Path;

use crate::cli::{
    Command, ConfigCommand, DoctorCommand, EpicCommand, FeaturesCommand, PhaseCommand,
    ReportCommand, SprintCommand, StoryCommand, TaskCommand, WebCommand,
};
use crate::dispatch::{DispatchMode, config_show_json_value, execute_command};
use crate::outcome::CommandOutcome;
use crate::theme::Theme;
use kanban_core::{
    ColorMode, ConfigGetDto, ConfigInitDto, ConfigSetDto, CreateStoryDto, DeleteStoryDto,
    DoctorDto, EpicShowDto, EpicUpdateDto, JsonEnvelope, KanbanErrorBody, KanbanErrorCode,
    ListIdsDto, MoveStoryDto, NoData, PhaseShowDto, PlanStoryDto, SprintCreateDto, SprintListDto,
    SprintOverviewDto, SprintRolloverDto, SprintSyncDto, StoryListDto, StoryShowDto,
    StoryUpdateDto, TaskMutationDto, TaskShowDto, ValidateDto,
};

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

fn json_kind(command: &Command) -> &'static str {
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
        Command::Phase {
            command: PhaseCommand::Show { .. },
        } => "phase.show",
        Command::Epic { command } => match command {
            EpicCommand::Show { .. } => "epic.show",
            EpicCommand::Update { .. } => "epic.update",
        },
        Command::Story { command } => match command {
            StoryCommand::Create { .. } => "story.create",
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
        json_kind(command),
        KanbanErrorBody::new(KanbanErrorCode::InvalidArgument, message),
    ))
}

fn emit_shared_outcome(outcome: CommandOutcome) -> i32 {
    let kind = outcome.kind();
    match outcome {
        CommandOutcome::Init(result) => {
            print_envelope(&JsonEnvelope::ok(kind, ConfigInitDto::from_result(&result)))
        }
        CommandOutcome::ConfigShow(raw) => match config_show_json_value(&raw) {
            Ok(value) => print_envelope(&JsonEnvelope::ok(kind, value)),
            Err(error) => print_envelope(&JsonEnvelope::<serde_json::Value>::error(
                kind,
                KanbanErrorBody::from_anyhow(&error),
            )),
        },
        CommandOutcome::ConfigGet { key, value } => {
            print_envelope(&JsonEnvelope::ok(kind, ConfigGetDto { key, value }))
        }
        CommandOutcome::ConfigSet(result) => {
            print_envelope(&JsonEnvelope::ok(kind, ConfigSetDto::from_result(&result)))
        }
        CommandOutcome::FeaturesList {
            phases,
            sprints,
            epics,
        } => print_envelope(&JsonEnvelope::ok(
            kind,
            serde_json::json!({ "phases": phases, "sprints": sprints, "epics": epics }),
        )),
        CommandOutcome::FeatureToggle { result, .. } => {
            print_envelope(&JsonEnvelope::ok(kind, ConfigSetDto::from_result(&result)))
        }
        CommandOutcome::Completion(dto) => print_envelope(&JsonEnvelope::ok(kind, dto)),
        CommandOutcome::ListIds {
            kind: item_kind,
            items,
        } => print_envelope(&JsonEnvelope::ok(kind, ListIdsDto::new(item_kind, items))),
        CommandOutcome::SprintOverview { sprint, .. } => print_envelope(&JsonEnvelope::ok(
            kind,
            SprintOverviewDto::from_overview(&sprint),
        )),
        CommandOutcome::SprintList {
            sprints,
            current_name,
        } => print_envelope(&JsonEnvelope::ok(
            kind,
            SprintListDto::new(&sprints, current_name.as_deref()),
        )),
        CommandOutcome::SprintCreate { result, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            SprintCreateDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::SprintRollover(result) => print_envelope(&JsonEnvelope::ok(
            kind,
            SprintRolloverDto::from_result(&result),
        )),
        CommandOutcome::PhaseShow(phase) => {
            print_envelope(&JsonEnvelope::ok(kind, PhaseShowDto::from_overview(&phase)))
        }
        CommandOutcome::EpicShow { id, result } => match *result {
            Some((details, source)) => print_envelope(&JsonEnvelope::ok(
                kind,
                EpicShowDto::from_details_and_source(&details, &source),
            )),
            None => print_envelope(&JsonEnvelope::<EpicShowDto>::error(
                kind,
                KanbanErrorBody::new(
                    KanbanErrorCode::EpicNotFound,
                    format!("No epic matches id '{id}'"),
                ),
            )),
        },
        CommandOutcome::StoryShow { id, result } => match *result {
            Some((details, source)) => print_envelope(&JsonEnvelope::ok(
                kind,
                StoryShowDto::from_details_and_source(&details, &source),
            )),
            None => print_envelope(&JsonEnvelope::<StoryShowDto>::error(
                kind,
                KanbanErrorBody::new(
                    KanbanErrorCode::StoryNotFound,
                    format!("No story matches id '{id}'"),
                ),
            )),
        },
        CommandOutcome::StoryCreate { result, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            CreateStoryDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::StoryList { scope, stories } => print_envelope(&JsonEnvelope::ok(
            kind,
            StoryListDto::new(scope.json_label(), &stories),
        )),
        CommandOutcome::TaskShow { details, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            TaskShowDto::from_result(&details, &repo_root),
        )),
        CommandOutcome::Validate(report) => {
            let dto = ValidateDto::from_report(&report, &report.repo_root);
            let env = if dto.valid {
                JsonEnvelope::ok(kind, dto)
            } else {
                JsonEnvelope::warning(kind, dto)
            };
            print_envelope(&env)
        }
        CommandOutcome::DoctorShow(findings) => {
            let dto = DoctorDto::from_findings(&findings);
            let env = if dto.healthy {
                JsonEnvelope::ok(kind, dto)
            } else {
                JsonEnvelope::warning(kind, dto)
            };
            print_envelope(&env)
        }
        CommandOutcome::ReportWbs(dto) => print_envelope(&JsonEnvelope::ok(kind, dto)),
        CommandOutcome::ReportForecast(dto) => print_envelope(&JsonEnvelope::ok(kind, dto)),
        CommandOutcome::StoryMove { result, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            MoveStoryDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::StoryPlan { result, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            PlanStoryDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::StoryDelete { result, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            DeleteStoryDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::EpicUpdate { result, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            EpicUpdateDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::StoryUpdate { result, repo_root } => print_envelope(&JsonEnvelope::ok(
            kind,
            StoryUpdateDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::TaskMutation {
            result, repo_root, ..
        } => print_envelope(&JsonEnvelope::ok(
            kind,
            TaskMutationDto::from_result(&result, &repo_root),
        )),
        CommandOutcome::SprintSync(changed) => print_envelope(&JsonEnvelope::ok(
            kind,
            SprintSyncDto::from_changed(changed),
        )),
        CommandOutcome::WebStatus(status) => print_envelope(&JsonEnvelope::ok(kind, status)),
        CommandOutcome::WebStart(started) => print_envelope(&JsonEnvelope::ok(kind, started)),
        CommandOutcome::WebStop(stopped) => print_envelope(&JsonEnvelope::ok(kind, stopped)),
        CommandOutcome::WebRestart(restarted) => print_envelope(&JsonEnvelope::ok(kind, restarted)),
        CommandOutcome::WebLog(log) => print_envelope(&JsonEnvelope::ok(kind, log)),
        CommandOutcome::Edited { .. } | CommandOutcome::NoData { .. } => {
            print_envelope(&JsonEnvelope::ok(kind, NoData))
        }
        CommandOutcome::Error { body, .. } => {
            print_envelope(&JsonEnvelope::<NoData>::error(kind, body))
        }
    }
}

/// Render a command through the shared execution path as a JSON envelope.
pub(crate) fn render_json(command: &Command) -> i32 {
    let theme = Theme::for_stdout(ColorMode::Never);
    match execute_command(command, DispatchMode::Json, &theme) {
        Ok(outcome) => emit_shared_outcome(outcome),
        Err(error) => print_envelope(&JsonEnvelope::<serde_json::Value>::error(
            json_kind(command),
            KanbanErrorBody::from_anyhow(&error),
        )),
    }
}

#[cfg(test)]
mod kind_consistency_tests {
    //! `json_kind` (this module, used for pre-execution error envelopes) and
    //! `CommandOutcome::kind()` (outcome.rs, used for successful/handled
    //! outcomes) both derive a `&'static str` "kind" label for the same
    //! commands. The two are hand-maintained in separate files, so they can
    //! silently drift. This test pins down agreement for a representative,
    //! cheap-to-construct subset of commands.
    //!
    //! Known exception (not covered here): `Command::Doctor(Show)` maps to
    //! `"doctor.show"` via `json_kind` but `CommandOutcome::DoctorShow(_)`
    //! reports `"doctor"` via `kind()`. That mismatch predates this test and
    //! is outside the scope of the fixes this test accompanies.
    use super::*;
    use crate::cli::{CompletionTarget, ConfigCommand, FeatureName, FeaturesCommand, ListIdsKind};
    use crate::outcome::CommandOutcome;
    use kanban_core::{CompletionDto, ConfigInitResult, ConfigSetResult};
    use std::path::PathBuf;

    fn assert_kinds_agree(label: &str, command: &Command, outcome: CommandOutcome) {
        assert_eq!(
            json_kind(command),
            outcome.kind(),
            "json_kind and CommandOutcome::kind() disagree for {label}"
        );
    }

    #[test]
    fn init_kinds_agree() {
        assert_kinds_agree(
            "init",
            &Command::Init {
                repo_root: PathBuf::from("."),
                no_sprints: false,
                no_epics: false,
                no_phases: false,
            },
            CommandOutcome::Init(ConfigInitResult {
                repo_root: PathBuf::from("."),
                config_dir: PathBuf::from(".kanban"),
                created_files: Vec::new(),
            }),
        );
    }

    #[test]
    fn config_kinds_agree() {
        assert_kinds_agree(
            "config.show",
            &Command::Config {
                command: ConfigCommand::Show {
                    repo_root: PathBuf::from("."),
                },
            },
            CommandOutcome::ConfigShow("{}".to_string()),
        );
        assert_kinds_agree(
            "config.get",
            &Command::Config {
                command: ConfigCommand::Get {
                    key: "theme.color_mode".to_string(),
                    repo_root: PathBuf::from("."),
                },
            },
            CommandOutcome::ConfigGet {
                key: "theme.color_mode".to_string(),
                value: "auto".to_string(),
            },
        );
        assert_kinds_agree(
            "config.set",
            &Command::Config {
                command: ConfigCommand::Set {
                    key: "theme.color_mode".to_string(),
                    value: "auto".to_string(),
                    repo_root: PathBuf::from("."),
                },
            },
            CommandOutcome::ConfigSet(ConfigSetResult {
                repo_root: PathBuf::from("."),
                file_path: PathBuf::from(".kanban/settings.json"),
                key: "theme.color_mode".to_string(),
                value: "auto".to_string(),
            }),
        );
    }

    #[test]
    fn features_kinds_agree() {
        assert_kinds_agree(
            "features.list",
            &Command::Features {
                command: FeaturesCommand::List {
                    repo_root: PathBuf::from("."),
                },
            },
            CommandOutcome::FeaturesList {
                phases: true,
                sprints: true,
                epics: true,
            },
        );
    }

    #[test]
    fn completion_kind_agrees() {
        assert_kinds_agree(
            "completion",
            &Command::Completion {
                target: CompletionTarget::Bash,
            },
            CommandOutcome::Completion(CompletionDto {
                target: "bash".to_string(),
                content_type: "shell-script".to_string(),
                content: String::new(),
            }),
        );
    }

    #[test]
    fn list_ids_kinds_agree() {
        assert_kinds_agree(
            "list-ids",
            &Command::ListIds {
                kind: ListIdsKind::Stories,
                repo_root: PathBuf::from("."),
            },
            CommandOutcome::ListIds {
                kind: "stories",
                items: Vec::new(),
            },
        );
        assert_kinds_agree(
            "list-task-ids",
            &Command::ListTaskIds {
                story_id: "US-F1-001".to_string(),
                repo_root: PathBuf::from("."),
            },
            CommandOutcome::ListIds {
                kind: "tasks",
                items: Vec::new(),
            },
        );
    }

    #[test]
    fn uninstall_and_upgrade_kinds_agree() {
        assert_kinds_agree(
            "uninstall",
            &Command::Uninstall {
                prefix: None,
                skills_dir: None,
                yes: false,
                dry_run: true,
                quiet: false,
            },
            CommandOutcome::NoData { kind: "uninstall" },
        );
        assert_kinds_agree(
            "upgrade",
            &Command::Upgrade {
                prefix: None,
                skills_dir: None,
                no_skills: false,
                yes: false,
                force: false,
                dry_run: true,
                quiet: false,
            },
            CommandOutcome::NoData { kind: "upgrade" },
        );
    }

    #[test]
    fn features_enable_disable_kinds_agree() {
        assert_kinds_agree(
            "features.enable",
            &Command::Features {
                command: FeaturesCommand::Enable {
                    feature: FeatureName::Sprints,
                    repo_root: PathBuf::from("."),
                },
            },
            CommandOutcome::FeatureToggle {
                action: crate::outcome::FeatureToggleAction::Enable,
                result: ConfigSetResult {
                    repo_root: PathBuf::from("."),
                    file_path: PathBuf::from(".kanban/settings.json"),
                    key: "features.sprints".to_string(),
                    value: "true".to_string(),
                },
            },
        );
        assert_kinds_agree(
            "features.disable",
            &Command::Features {
                command: FeaturesCommand::Disable {
                    feature: FeatureName::Sprints,
                    repo_root: PathBuf::from("."),
                },
            },
            CommandOutcome::FeatureToggle {
                action: crate::outcome::FeatureToggleAction::Disable,
                result: ConfigSetResult {
                    repo_root: PathBuf::from("."),
                    file_path: PathBuf::from(".kanban/settings.json"),
                    key: "features.sprints".to_string(),
                    value: "false".to_string(),
                },
            },
        );
    }
}
