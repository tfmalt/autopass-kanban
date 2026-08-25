use std::path::PathBuf;

use kanban_core::{
    CompletionDto, ConfigInitResult, ConfigSetResult, CreateSprintResult, CreateStoryResult,
    DeleteStoryResult, DoctorFinding, Epic, EpicDetails, EpicUpdateResult, KanbanErrorBody,
    ListIdItemDto, MoveStoryResult, PhaseOverview, PlanStoryResult, ReportForecastDto,
    ReportWbsDto, RolloverResult, SprintOverview, SprintUpdateResult, Story, StoryDetails,
    StoryOverview, StoryUpdateResult, TaskListResult, TaskMutationResult, ValidationReport,
};

use crate::layout::OutputLayout;
use crate::ops::StoryListScope;
use crate::render::common::format_story_points;
use crate::render::epic::print_epic_details;
use crate::render::phase::print_phase_overview;
use crate::render::sprint::{print_sprint_overview, print_sprint_overview_short};
use crate::render::story::{print_story_details, print_story_list, render_task_list};
use crate::theme::Theme;
use crate::web::{WebLogDto, WebRestartDto, WebStartDto, WebStatusDto, WebStopDto};
use crate::{doctor_cli::print_doctor_findings, theme::Style};

pub(crate) enum FeatureToggleAction {
    Enable,
    Disable,
}

pub(crate) enum CommandOutcome {
    Init(ConfigInitResult),
    ConfigShow(String),
    ConfigGet {
        key: String,
        value: String,
    },
    ConfigSet(ConfigSetResult),
    FeaturesList {
        phases: bool,
        sprints: bool,
        epics: bool,
    },
    FeatureToggle {
        action: FeatureToggleAction,
        result: ConfigSetResult,
    },
    Completion(CompletionDto),
    ListIds {
        kind: &'static str,
        items: Vec<ListIdItemDto>,
    },
    SprintOverview {
        kind: &'static str,
        sprint: SprintOverview,
        short: bool,
    },
    SprintList {
        sprints: Vec<SprintOverview>,
        current_name: Option<String>,
    },
    SprintCreate {
        result: CreateSprintResult,
        repo_root: PathBuf,
    },
    SprintUpdate {
        result: SprintUpdateResult,
        repo_root: PathBuf,
    },
    SprintRollover(RolloverResult),
    PhaseShow(PhaseOverview),
    EpicShow {
        id: String,
        result: Box<Option<(EpicDetails, Epic)>>,
    },
    StoryShow {
        id: String,
        result: Box<Option<(StoryDetails, Story)>>,
    },
    StoryCreate {
        result: CreateStoryResult,
        repo_root: PathBuf,
    },
    StoryList {
        scope: StoryListScope,
        stories: Vec<StoryOverview>,
    },
    TaskShow {
        details: TaskListResult,
        repo_root: PathBuf,
    },
    Validate(ValidationReport),
    DoctorShow(Vec<DoctorFinding>),
    ReportWbs(Box<ReportWbsDto>),
    ReportForecast(ReportForecastDto),
    StoryMove {
        result: MoveStoryResult,
        repo_root: PathBuf,
    },
    StoryPlan {
        result: PlanStoryResult,
        repo_root: PathBuf,
    },
    StoryDelete {
        result: DeleteStoryResult,
        repo_root: PathBuf,
    },
    EpicUpdate {
        result: EpicUpdateResult,
        repo_root: PathBuf,
    },
    StoryUpdate {
        result: StoryUpdateResult,
        repo_root: PathBuf,
    },
    TaskMutation {
        kind: &'static str,
        result: TaskMutationResult,
        repo_root: PathBuf,
    },
    SprintSync(Vec<String>),
    WebStatus(WebStatusDto),
    WebStart(WebStartDto),
    WebStop(WebStopDto),
    WebRestart(WebRestartDto),
    WebLog(WebLogDto),
    Edited {
        kind: &'static str,
        path: PathBuf,
    },
    NoData {
        kind: &'static str,
    },
    Error {
        kind: &'static str,
        body: KanbanErrorBody,
    },
}

impl CommandOutcome {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            CommandOutcome::Init(_) => "init",
            CommandOutcome::ConfigShow(_) => "config.show",
            CommandOutcome::ConfigGet { .. } => "config.get",
            CommandOutcome::ConfigSet(_) => "config.set",
            CommandOutcome::FeaturesList { .. } => "features.list",
            CommandOutcome::FeatureToggle { action, .. } => match action {
                FeatureToggleAction::Enable => "features.enable",
                FeatureToggleAction::Disable => "features.disable",
            },
            CommandOutcome::Completion(_) => "completion",
            CommandOutcome::ListIds { kind, .. } => {
                if *kind == "tasks" {
                    "list-task-ids"
                } else {
                    "list-ids"
                }
            }
            CommandOutcome::SprintOverview { kind, .. } => kind,
            CommandOutcome::SprintList { .. } => "sprint.list",
            CommandOutcome::SprintCreate { .. } => "sprint.create",
            CommandOutcome::SprintUpdate { .. } => "sprint.update",
            CommandOutcome::SprintRollover(_) => "sprint.rollover",
            CommandOutcome::PhaseShow(_) => "phase.show",
            CommandOutcome::EpicShow { .. } => "epic.show",
            CommandOutcome::StoryShow { .. } => "story.show",
            CommandOutcome::StoryCreate { .. } => "story.create",
            CommandOutcome::StoryList { .. } => "story.list",
            CommandOutcome::TaskShow { .. } => "task.show",
            CommandOutcome::Validate(_) => "validate",
            CommandOutcome::DoctorShow(_) => "doctor",
            CommandOutcome::ReportWbs(_) => "report.wbs",
            CommandOutcome::ReportForecast(_) => "report.forecast",
            CommandOutcome::StoryMove { .. } => "story.move",
            CommandOutcome::StoryPlan { .. } => "story.plan",
            CommandOutcome::StoryDelete { .. } => "story.delete",
            CommandOutcome::EpicUpdate { .. } => "epic.update",
            CommandOutcome::StoryUpdate { .. } => "story.update",
            CommandOutcome::TaskMutation { kind, .. } => kind,
            CommandOutcome::SprintSync(_) => "sprint.sync",
            CommandOutcome::WebStatus(_) => "web.status",
            CommandOutcome::WebStart(_) => "web.start",
            CommandOutcome::WebStop(_) => "web.stop",
            CommandOutcome::WebRestart(_) => "web.restart",
            CommandOutcome::WebLog(_) => "web.log",
            CommandOutcome::Edited { kind, .. }
            | CommandOutcome::NoData { kind }
            | CommandOutcome::Error { kind, .. } => kind,
        }
    }
}

pub(crate) fn print_human_outcome(theme: &Theme, outcome: CommandOutcome) {
    match outcome {
        CommandOutcome::Init(result) => {
            println!(
                "{} initialized config: {}",
                theme.ok_label(),
                theme.path(result.config_dir.display())
            );
            if result.created_files.is_empty() {
                println!("{} created files: none", theme.info_label());
            } else {
                for file in result.created_files {
                    println!(
                        "{} created file: {}",
                        theme.info_label(),
                        theme.path(file.display())
                    );
                }
            }
        }
        CommandOutcome::ConfigShow(value) => println!("{value}"),
        CommandOutcome::ConfigGet { value, .. } => println!("{value}"),
        CommandOutcome::ConfigSet(result) => {
            println!(
                "{} updated {} = {}",
                theme.ok_label(),
                theme.id(&result.key),
                result.value
            );
            println!(
                "{} file: {}",
                theme.info_label(),
                theme.path(result.file_path.display())
            );
        }
        CommandOutcome::FeaturesList {
            phases,
            sprints,
            epics,
        } => {
            println!("{}", theme.heading("Features"));
            for (name, enabled) in [("phases", phases), ("sprints", sprints), ("epics", epics)] {
                let status = if enabled { "on" } else { "off" };
                println!("  {name:8} {status}");
            }
        }
        CommandOutcome::FeatureToggle { action, result } => {
            let verb = match action {
                FeatureToggleAction::Enable => "enabled",
                FeatureToggleAction::Disable => "disabled",
            };
            println!(
                "{} {verb} {} = {}",
                theme.ok_label(),
                theme.id(&result.key),
                result.value
            );
        }
        CommandOutcome::Completion(dto) => {
            if dto.content_type == "help" {
                println!("{}", dto.content);
            } else {
                print!("{}", dto.content);
            }
        }
        CommandOutcome::ListIds { items, .. } => {
            for item in items {
                if let Some(description) = item.description {
                    let description = description.replace(['\t', '\n', '\r'], " ");
                    println!("{}\t{}", item.value, description);
                } else {
                    println!("{}", item.value);
                }
            }
        }
        CommandOutcome::SprintOverview { sprint, short, .. } => {
            if short {
                print_sprint_overview_short(theme, OutputLayout::for_stdout().unwrap(), &sprint);
            } else {
                print_sprint_overview(theme, OutputLayout::for_stdout().unwrap(), &sprint);
            }
        }
        CommandOutcome::SprintList { sprints, .. } => {
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
        CommandOutcome::SprintCreate { result, .. } => {
            println!(
                "{} created sprint: {}",
                theme.ok_label(),
                result.sprint_name
            );
            println!(
                "{} path: {}",
                theme.info_label(),
                theme.path(result.sprint_path.display())
            );
        }
        CommandOutcome::SprintUpdate { result, .. } => {
            println!(
                "{} updated {} ({})",
                theme.ok_label(),
                theme.id(&result.sprint_name),
                result.updated_fields.join(", ")
            );
            println!(
                "{} path: {}",
                theme.info_label(),
                theme.path(result.sprint_path.display())
            );
        }
        CommandOutcome::SprintRollover(result) => {
            crate::prompt::print_rollover_result(theme, &result);
        }
        CommandOutcome::PhaseShow(phase) => {
            print_phase_overview(theme, OutputLayout::for_stdout().unwrap(), &phase);
        }
        CommandOutcome::EpicShow { id, result } => match *result {
            Some((details, _source)) => {
                print_epic_details(theme, OutputLayout::for_stdout().unwrap(), &details)
            }
            None => println!("{} epic not found: {id}", theme.warning_label()),
        },
        CommandOutcome::StoryShow { id, result } => match *result {
            Some((details, _source)) => {
                print_story_details(theme, OutputLayout::for_stdout().unwrap(), &details)
            }
            None => println!("{} story not found: {id}", theme.warning_label()),
        },
        CommandOutcome::StoryCreate { result, .. } => {
            println!(
                "{} created {}",
                theme.ok_label(),
                theme.id(&result.story_id)
            );
            println!("{} epic: {}", theme.info_label(), theme.id(&result.epic_id));
            if let Some(sprint_name) = result.sprint_name {
                println!("{} sprint: {}", theme.info_label(), sprint_name);
            }
            println!(
                "{} story: {}",
                theme.info_label(),
                theme.path(result.story_path.display())
            );
        }
        CommandOutcome::StoryList { scope, stories } => {
            print_story_list(theme, &scope.human_label(), &stories);
        }
        CommandOutcome::TaskShow { details, .. } => {
            print!(
                "{}",
                render_task_list(
                    theme,
                    OutputLayout::for_stdout().unwrap(),
                    &details.story_id,
                    details.task_file_path.as_deref(),
                    &details.tasks,
                )
            );
        }
        CommandOutcome::Validate(report) => {
            if report.issues.is_empty() {
                println!("{} no validation issues found.", theme.ok_label());
            } else {
                for issue in report.issues {
                    println!(
                        "{} {} [{}] {}",
                        theme.warning_label(),
                        theme.path(issue.file_path.display()),
                        theme.warning(issue.rule),
                        issue.message
                    );
                }
            }
        }
        CommandOutcome::DoctorShow(findings) => print_doctor_findings(theme, &findings),
        CommandOutcome::ReportWbs(dto) => print_report_wbs(theme, *dto),
        CommandOutcome::ReportForecast(dto) => print_report_forecast(theme, dto),
        CommandOutcome::StoryMove { result, .. } => {
            println!(
                "{} moved {} in {}: {} -> {}",
                theme.ok_label(),
                theme.id(&result.story_id),
                result.sprint_name,
                theme.status(&result.from_status),
                theme.status(&result.to_status)
            );
            println!(
                "{} story: {}",
                theme.info_label(),
                theme.path(result.story_path.display())
            );
            if let Some(task_path) = result.task_path {
                println!(
                    "{} task file: {}",
                    theme.info_label(),
                    theme.path(task_path.display())
                );
            }
        }
        CommandOutcome::StoryPlan { result, .. } => {
            println!(
                "{} planned {} -> {}",
                theme.ok_label(),
                theme.id(&result.story_id),
                result.sprint_name
            );
            println!(
                "{} story: {}",
                theme.info_label(),
                theme.path(result.story_path.display())
            );
            if let Some(task_path) = result.task_path {
                println!(
                    "{} tasks: {}",
                    theme.info_label(),
                    theme.path(task_path.display())
                );
            }
        }
        CommandOutcome::StoryDelete { result, .. } => {
            println!(
                "{} deleted {}",
                theme.ok_label(),
                theme.id(&result.story_id)
            );
            println!(
                "{} story: {}",
                theme.info_label(),
                theme.path(result.story_path.display())
            );
            if let Some(task_path) = result.task_path {
                println!(
                    "{} tasks: {}",
                    theme.info_label(),
                    theme.path(task_path.display())
                );
            }
            if let Some(sprint_name) = result.sprint_name {
                println!("{} updated sprint: {}", theme.info_label(), sprint_name);
            }
        }
        CommandOutcome::EpicUpdate { result, .. } => {
            println!(
                "{} updated {} ({})",
                theme.ok_label(),
                theme.id(&result.epic_id),
                result.updated_fields.join(", ")
            );
            println!(
                "{} epic: {}",
                theme.info_label(),
                theme.path(result.epic_path.display())
            );
        }
        CommandOutcome::StoryUpdate { result, .. } => {
            println!(
                "{} updated {} ({})",
                theme.ok_label(),
                theme.id(&result.story_id),
                result.updated_fields.join(", ")
            );
            println!(
                "{} story: {}",
                theme.info_label(),
                theme.path(result.story_path.display())
            );
        }
        CommandOutcome::TaskMutation { kind, result, .. } => {
            let verb = match kind {
                "task.add" => "added",
                "task.update" => "updated",
                "task.delete" => "deleted",
                _ => "updated",
            };
            let join = if kind == "task.delete" {
                " from "
            } else {
                " to "
            };
            if kind == "task.update" {
                println!(
                    "{} updated {} in {}",
                    theme.ok_label(),
                    theme.id(&result.task_id),
                    theme.id(&result.story_id)
                );
            } else {
                println!(
                    "{} {verb} {}{join}{}",
                    theme.ok_label(),
                    theme.id(&result.task_id),
                    theme.id(&result.story_id)
                );
            }
            println!(
                "{} task file: {}",
                theme.info_label(),
                theme.path(result.task_file_path.display())
            );
        }
        CommandOutcome::SprintSync(changed) => {
            if changed.is_empty() {
                println!(
                    "{} sprint story tables are already up to date.",
                    theme.ok_label()
                );
            } else {
                println!("{} regenerated sprint story tables:", theme.ok_label());
                for sprint in changed {
                    println!("{} sprint: {}", theme.info_label(), theme.id(sprint));
                }
            }
        }
        CommandOutcome::WebStatus(status) => print_web_status_dto(theme, status),
        CommandOutcome::WebStart(started) => {
            println!(
                "{} started kanban web UI: {}",
                theme.ok_label(),
                started.url
            );
            println!("{} pid: {}", theme.info_label(), started.pid);
            println!(
                "{} log: {}",
                theme.info_label(),
                theme.path(&started.log_file)
            );
        }
        CommandOutcome::WebStop(stopped) => {
            if stopped.stopped {
                println!("{} stopped web UI.", theme.ok_label());
            } else {
                println!("{} web UI is not running.", theme.info_label());
            }
        }
        CommandOutcome::WebRestart(restarted) => {
            if restarted.stopped_existing {
                println!("{} stopped existing web UI.", theme.info_label());
            }
            println!(
                "{} started kanban web UI: {}",
                theme.ok_label(),
                restarted.started.url
            );
            println!("{} pid: {}", theme.info_label(), restarted.started.pid);
            println!(
                "{} log: {}",
                theme.info_label(),
                theme.path(&restarted.started.log_file)
            );
        }
        CommandOutcome::WebLog(log) => {
            if log.exists {
                print!("{}", log.content);
                if !log.content.is_empty() && !log.content.ends_with('\n') {
                    println!();
                }
            } else {
                println!(
                    "{} no web log found: {}",
                    theme.warning_label(),
                    theme.path(&log.path)
                );
            }
        }
        CommandOutcome::Edited { path, .. } => {
            println!("{} edited {}", theme.ok_label(), theme.path(path.display()));
        }
        CommandOutcome::NoData { .. } => {}
        CommandOutcome::Error { body, .. } => {
            eprintln!(
                "{} {}",
                theme.error_label(),
                theme.highlight_commands(&body.message)
            );
        }
    }
}

fn print_web_status_dto(theme: &Theme, status: WebStatusDto) {
    match status.state.as_str() {
        "running" => {
            println!("{} web UI: running", theme.ok_label());
            if let Some(pid) = status.pid {
                println!("{} pid: {pid}", theme.info_label());
            }
            println!("{} url: {}", theme.info_label(), status.url);
            println!(
                "{} log: {}",
                theme.info_label(),
                theme.path(&status.log_file)
            );
        }
        "stale" => {
            match status.stale_pid {
                Some(pid) => println!("{} web UI: stale PID {pid}", theme.warning_label()),
                None => println!("{} web UI: stale PID file", theme.warning_label()),
            }
            println!(
                "{} pid file: {}",
                theme.info_label(),
                theme.path(&status.pid_file)
            );
        }
        _ => {
            println!("{} web UI: stopped", theme.info_label());
            println!("{} url: {}", theme.info_label(), status.url);
        }
    }
}

fn print_report_wbs(theme: &Theme, dto: ReportWbsDto) {
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

fn print_report_forecast(theme: &Theme, dto: ReportForecastDto) {
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
