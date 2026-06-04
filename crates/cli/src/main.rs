use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, IsTerminal, Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use clap::builder::styling::{AnsiColor, Effects, Style as ClapStyle, Styles};
use clap::{ArgGroup, CommandFactory, Parser, Subcommand, ValueEnum};
use kanban_core::{
    ColorMode, CompletionDto, ConfigGetDto, ConfigInitDto, ConfigSetDto, CreateSprintInput,
    DoctorDto, DoctorFinding, DoctorFixInput, DoctorFixKind, DoctorIssue, DoctorPrompt,
    JsonEnvelope, KanbanErrorBody, KanbanErrorCode, ListIdItemDto, ListIdsDto, MoveStoryDto,
    NoData, PhaseOverview, PhaseShowDto, PlanStoryDto, RolloverResult, SprintCreateDto,
    SprintListDto, SprintOverview, SprintOverviewDto, SprintRolloverDto, SprintSyncDto,
    StoryDetails, StoryListDto, StoryOverview, StoryShowDto, StoryUpdateDto, TaskMutationDto,
    TaskSummary, ValidateDto, add_task_to_story, apply_doctor_fix, collect_doctor_issues,
    collect_doctor_issues_for_current_sprint, collect_doctor_issues_for_story, config_show_value,
    create_sprint, doctor_repository, find_story, find_story_with_source, get_config_json,
    get_config_value, init_config, list_all_stories, list_current_sprint_stories, list_epic_ids,
    list_next_sprint_stories, list_sprint_names, list_stories_in_sprint,
    list_story_completion_items, list_story_ids, load_kanban_config,
    move_story_to_status_with_assignee, plan_story_into_sprint, read_story_file, rollover_sprint,
    set_config_value, story_markdown_file, suggested_next_sprint_dates,
    suggested_next_sprint_number, suggested_sprint_dates, summarize_current_sprint,
    summarize_phase, summarize_sprint, summarize_sprints, sync_sprint_rosters,
    update_story_frontmatter, update_task_in_story, validate_repository,
};
use serde::Serialize;

const MIN_TERMINAL_WIDTH: usize = 80;
const DEFAULT_OUTPUT_WIDTH: usize = 100;
const SPRINT_CONTENT_INSET: usize = 2;
const SPRINT_STORY_ROW_PREFIX: &str = "    · ";
const CLAP_STYLING: Styles = Styles::styled()
    .header(
        ClapStyle::new()
            .bold()
            .underline()
            .fg_color(Some(clap::builder::styling::Color::Ansi(AnsiColor::Cyan))),
    )
    .usage(
        ClapStyle::new()
            .bold()
            .underline()
            .fg_color(Some(clap::builder::styling::Color::Ansi(AnsiColor::Blue))),
    )
    .literal(
        ClapStyle::new()
            .bold()
            .fg_color(Some(clap::builder::styling::Color::Ansi(AnsiColor::Green))),
    )
    .placeholder(
        ClapStyle::new().fg_color(Some(clap::builder::styling::Color::Ansi(
            AnsiColor::Magenta,
        ))),
    )
    .error(
        ClapStyle::new()
            .bold()
            .fg_color(Some(clap::builder::styling::Color::Ansi(AnsiColor::Red))),
    )
    .valid(
        ClapStyle::new()
            .effects(Effects::BOLD)
            .fg_color(Some(clap::builder::styling::Color::Ansi(AnsiColor::Green))),
    )
    .invalid(
        ClapStyle::new()
            .effects(Effects::BOLD)
            .fg_color(Some(clap::builder::styling::Color::Ansi(AnsiColor::Yellow))),
    )
    .context(
        ClapStyle::new().fg_color(Some(clap::builder::styling::Color::Ansi(
            AnsiColor::BrightBlack,
        ))),
    );

#[derive(Copy, Clone)]
struct Theme {
    color: bool,
}

#[derive(Copy, Clone)]
enum Style {
    Bold,
    DarkGray,
    Muted,
    Blue,
    Cyan,
    Green,
    Purple,
    Red,
    Yellow,
}

impl Theme {
    fn for_stdout(color_mode: ColorMode) -> Self {
        Self {
            color: match color_mode {
                ColorMode::Always => true,
                ColorMode::Never => false,
                ColorMode::Auto => {
                    std::io::stdout().is_terminal()
                        && std::env::var_os("NO_COLOR").is_none()
                        && std::env::var_os("TERM").is_none_or(|term| term != "dumb")
                }
            },
        }
    }

    #[cfg(test)]
    fn color() -> Self {
        Self { color: true }
    }

    #[cfg(test)]
    fn plain() -> Self {
        Self { color: false }
    }

    fn paint(&self, style: Style, value: impl std::fmt::Display) -> String {
        if !self.color {
            return value.to_string();
        }

        let code = foreground_code(style);
        format!("\x1b[{code}m{value}\x1b[0m")
    }

    fn paint_with_background(
        &self,
        foreground: Style,
        background: Style,
        value: impl std::fmt::Display,
    ) -> String {
        if !self.color {
            return value.to_string();
        }

        format!(
            "\x1b[{};{}m{value}\x1b[0m",
            foreground_code(foreground),
            background_code(background)
        )
    }

    fn heading(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Bold, value)
    }

    fn label(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Bold, value)
    }

    fn id(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Cyan, value)
    }

    fn count(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Bold, value)
    }

    fn story_points(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Yellow, value)
    }

    fn path(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Muted, value)
    }

    fn success(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Green, value)
    }

    fn warning(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Yellow, value)
    }

    fn error(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Red, value)
    }

    fn command(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Blue, value)
    }

    fn highlight(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Purple, value)
    }

    fn status(&self, status: &str) -> String {
        match status {
            "todo" => self.paint(Style::Muted, status),
            "in-progress" => self.paint(Style::Blue, status),
            "ready-for-qa" => self.paint(Style::Purple, status),
            "done" => self.paint(Style::Green, status),
            "blocked" => self.paint(Style::Red, status),
            _ => status.to_string(),
        }
    }

    fn status_text(&self, status: &str, text: impl std::fmt::Display) -> String {
        match status {
            "todo" => self.paint(Style::Muted, text),
            "in-progress" => self.paint(Style::Blue, text),
            "ready-for-qa" => self.paint(Style::Purple, text),
            "done" => self.paint(Style::Green, text),
            "blocked" => self.paint(Style::Red, text),
            _ => text.to_string(),
        }
    }

    fn severity(&self, severity: &str) -> String {
        match severity.to_ascii_lowercase().as_str() {
            "error" | "critical" => self.paint(Style::Red, severity),
            "warning" | "warn" => self.paint(Style::Yellow, severity),
            "info" => self.paint(Style::Cyan, severity),
            _ => severity.to_string(),
        }
    }
}

fn foreground_code(style: Style) -> &'static str {
    match style {
        Style::Bold => "1",
        Style::DarkGray => "90",
        Style::Muted => "2",
        Style::Blue => "1;34",
        Style::Cyan => "1;36",
        Style::Green => "1;32",
        Style::Purple => "1;35",
        Style::Red => "1;31",
        Style::Yellow => "1;33",
    }
}

fn background_code(style: Style) -> &'static str {
    match style {
        Style::Bold | Style::Muted => "100",
        Style::DarkGray => "40",
        Style::Blue => "44",
        Style::Cyan => "46",
        Style::Green => "42",
        Style::Purple => "45",
        Style::Red => "41",
        Style::Yellow => "43",
    }
}

#[derive(Copy, Clone)]
struct OutputLayout {
    width: usize,
}

impl OutputLayout {
    fn for_stdout() -> Result<Self> {
        let width = detected_terminal_width().unwrap_or(DEFAULT_OUTPUT_WIDTH);
        if width < MIN_TERMINAL_WIDTH {
            bail!(
                "Terminal width must be at least {MIN_TERMINAL_WIDTH} columns for kanban output; detected {width}."
            );
        }
        Ok(Self { width })
    }
}

fn detected_terminal_width() -> Option<usize> {
    if std::io::stdout().is_terminal() {
        terminal_width_from_stdout().or_else(terminal_width_from_columns)
    } else {
        terminal_width_from_columns()
    }
}

fn terminal_width_from_columns() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > 0)
}

#[cfg(unix)]
fn terminal_width_from_stdout() -> Option<usize> {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::zeroed();
    let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, size.as_mut_ptr()) };
    if result != 0 {
        return None;
    }

    let size = unsafe { size.assume_init() };
    (size.ws_col > 0).then_some(size.ws_col as usize)
}

#[cfg(not(unix))]
fn terminal_width_from_stdout() -> Option<usize> {
    None
}

#[derive(Parser)]
#[command(name = "kanban")]
#[command(bin_name = "kanban")]
#[command(version)]
#[command(visible_alias = "kb")]
#[command(about = "Markdown-first kanban tooling")]
#[command(styles = CLAP_STYLING)]
#[command(max_term_width = 100)]
#[command(
    long_about = "Markdown-first kanban tooling for the AutoPASS IP 2.0 backlog. Commands state whether they are read-only or which markdown files they mutate."
)]
struct Args {
    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "human",
        help = "Output format. `json` emits a single machine-readable envelope; human output is the default."
    )]
    format: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum SprintCommand {
    #[command(
        about = "Show the current sprint. Effect: read-only inspection of sprint files and metadata. Side effects: none."
    )]
    Current {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "List sprint files. Effect: read-only inspection of the configured sprint path from `.kanban/paths.json`. Side effects: none."
    )]
    List {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Show one sprint summary. Defaults to the current sprint when NAME is omitted. Effect: read-only inspection of the selected sprint file and frontmatter-derived stories/tasks. Side effects: none."
    )]
    Show {
        #[arg(
            help = "Sprint name to inspect, for example S001.foundation. Defaults to the current sprint."
        )]
        name: Option<String>,
        #[arg(long, help = "Only print the sprint status header.")]
        short: bool,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Create a sprint file. Effect: writes one S###.slug.md file under the configured sprint path from `.kanban/paths.json`. Side effects: prompts for metadata unless --non-interactive or at least one of --number/--headline/--start/--end is supplied.",
        long_about = "Create a sprint file. Effect: writes one S###.slug.md file under the configured sprint path from `.kanban/paths.json`. Side effects: prompts for metadata unless --non-interactive or at least one of --number/--headline/--start/--end is supplied.\n\nNon-interactive behavior:\n  `--headline` is required whenever flags are used to build the sprint without prompts.\n  `--number` defaults to the next suggested sprint number.\n  `--start` defaults to the suggested next start date, or today if no sprint history exists.\n  `--end` defaults to the suggested next end date, or a derived end date from the chosen start date.\n\nExample:\n  kanban sprint create --non-interactive --headline foundation --start 2026-06-01 --end 2026-06-12"
    )]
    Create {
        #[arg(
            long,
            value_name = "N",
            help = "Sprint number. Defaults to the next suggested number."
        )]
        number: Option<u32>,
        #[arg(
            long,
            value_name = "SLUG",
            help = "Sprint headline slug. Required in non-interactive mode."
        )]
        headline: Option<String>,
        #[arg(
            long,
            value_name = "YYYY-MM-DD",
            help = "Start date. Defaults to the suggested next start date."
        )]
        start: Option<String>,
        #[arg(
            long,
            value_name = "YYYY-MM-DD",
            help = "End date. Defaults to the suggested next end date."
        )]
        end: Option<String>,
        #[arg(
            long,
            help = "Do not prompt; build the sprint from flags and suggested defaults."
        )]
        non_interactive: bool,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Roll unfinished work into the next sprint. Effect: updates story sprint frontmatter and the closed sprint file. Side effects: may create the next sprint file."
    )]
    Rollover {
        #[arg(help = "Sprint name to close and roll over.")]
        name: String,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Regenerate generated story rosters in all sprint files. Effect: rewrites only generated ## Stories blocks."
    )]
    Sync {
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum PhaseCommand {
    #[command(
        about = "Show phase backlog state. Effect: read-only inspection of phase backlog stories and sprint assignments. Side effects: none."
    )]
    Show {
        #[arg(help = "Phase identifier to inspect, for example 1 or F1.")]
        phase: String,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum StoryCommand {
    #[command(
        about = "Show one story. Effect: read-only inspection of the canonical story file plus acceptance criteria and tasks. Side effects: none."
    )]
    Show {
        #[arg(help = "Story id to inspect, for example US-F1-053.")]
        id: String,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "List stories. Effect: read-only inspection of canonical story files across backlog phases and sprint assignments. Side effects: none."
    )]
    #[command(group(
        ArgGroup::new("scope")
            .args(["current", "all", "next", "sprint"])
            .multiple(false)
    ))]
    List {
        #[arg(long, help = "List stories in the current or active sprint.")]
        current: bool,
        #[arg(long, help = "List all stories across the configured backlog root.")]
        all: bool,
        #[arg(
            long,
            help = "List stories in the next sprint after the current sprint."
        )]
        next: bool,
        #[arg(
            long,
            value_name = "ID",
            help = "List stories assigned to the specified sprint, for example S001.foundation."
        )]
        sprint: Option<String>,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Move a story to another status. Effect: updates the canonical story frontmatter and regenerates the sprint roster. Side effects: in-progress sets assignee/work_started; done refreshes work_done."
    )]
    Move {
        #[arg(help = "Story id to move, for example US-F1-053.")]
        id: String,
        #[arg(
            help = "Target status, for example todo, in-progress, ready-for-qa, done, or blocked."
        )]
        status: String,
        #[arg(
            short,
            long,
            value_name = "NAME <EMAIL>",
            help = "Override assignee when moving to in-progress. Must use the exact structure `Name <email>`; invalid values fail before files are moved."
        )]
        assignee: Option<String>,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Plan a backlog story into a sprint. Effect: updates the canonical story frontmatter (status=todo, sprint, activated, updated) and regenerates the sprint roster. Side effects: none beyond those markdown updates."
    )]
    Plan {
        #[arg(help = "Backlog story id to plan, for example US-F2-001.")]
        id: String,
        #[arg(
            long,
            value_name = "SPRINT",
            help = "Target sprint name or Snnn prefix, for example S001.planning or S001."
        )]
        sprint: String,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Update a story. With no field options, opens $EDITOR for the story markdown. Field options update frontmatter; omit an option value to be prompted with the current value as default."
    )]
    Update {
        #[arg(help = "Story id to update, for example US-F1-053.")]
        id: String,
        #[arg(long = "id", num_args = 0..=1, value_name = "ID", help = "Update frontmatter id. Omit VALUE to prompt with the current value.")]
        frontmatter_id: Option<Option<String>>,
        #[arg(long = "type", num_args = 0..=1, value_name = "TYPE", help = "Update frontmatter type. Omit VALUE to prompt with the current value.")]
        story_type: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "STATUS", help = "Update frontmatter status. Omit VALUE to prompt with the current value.")]
        status: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "EPIC", help = "Update frontmatter epic. Omit VALUE to prompt with the current value.")]
        epic: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "SPRINT", help = "Update frontmatter sprint. Omit VALUE to prompt with the current value.")]
        sprint: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "POINTS", help = "Update frontmatter story_points. Omit VALUE to prompt with the current value.")]
        story_points: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "ASSIGNEE", help = "Update frontmatter assignee. Omit VALUE to prompt with the current value.")]
        assignee: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "TIMESTAMP", help = "Update frontmatter activated. Omit VALUE to prompt with the current value.")]
        activated: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "TIMESTAMP", help = "Update frontmatter work_started. Omit VALUE to prompt with the current value.")]
        work_started: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "TIMESTAMP", help = "Update frontmatter work_done. Omit VALUE to prompt with the current value.")]
        work_done: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "TIMESTAMP", help = "Update frontmatter created. Omit VALUE to prompt with the current value.")]
        created: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "TIMESTAMP", help = "Update frontmatter updated. Omit VALUE to prompt with the current value.")]
        updated: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "PATH", help = "Update frontmatter task_file. Omit VALUE to prompt with the current value.")]
        task_file: Option<Option<String>>,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum TaskCommand {
    #[command(
        about = "Add a sprint task. Effect: appends a task block to the story's sibling .tasks.md file. Side effects: does not create standalone T-*.md files."
    )]
    Add {
        #[arg(help = "Parent story id for the task, for example US-F1-053.")]
        story_id: String,
        #[arg(long, help = "Task title to append to the sibling task log.")]
        title: String,
        #[arg(
            long,
            default_value = "todo",
            help = "Initial task status to write. Defaults to todo."
        )]
        status: String,
        #[arg(
            long,
            value_delimiter = ',',
            help = "Comma-separated task tags to write."
        )]
        tags: Vec<String>,
        #[arg(long, help = "Task description to write in the task log.")]
        description: String,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Update a sprint task. Effect: rewrites the matching task block in the story's sibling .tasks.md file. Side effects: only supplied fields are changed."
    )]
    Update {
        #[arg(help = "Parent story id for the task, for example US-F1-053.")]
        story_id: String,
        #[arg(help = "Task id to update, for example TASK-US-F1-053-001.")]
        task_id: String,
        #[arg(
            long,
            help = "Replacement task title. Omitted means keep the current title."
        )]
        title: Option<String>,
        #[arg(
            long,
            help = "Replacement task status. Omitted means keep the current status."
        )]
        status: Option<String>,
        #[arg(
            long,
            value_delimiter = ',',
            help = "Replacement comma-separated task tags. Omitted means keep current tags."
        )]
        tags: Option<Vec<String>>,
        #[arg(
            long,
            help = "Replacement task description. Omitted means keep the current description."
        )]
        description: Option<String>,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

const COMPLETION_HELP: &str = "Generate a shell completion script from the current kanban command tree.\n\nInstall zsh completion — add to ~/.zshrc:\n  eval \"$(kanban completion zsh)\"\n\nInstall bash completion — add to ~/.bashrc or ~/.bash_profile:\n  eval \"$(kanban completion bash)\"\n\nNote on direnv: .envrc is evaluated as bash, so eval \"$(kanban completion zsh)\" cannot\nbe placed there. Add the eval line to ~/.zshrc instead; it runs once per shell.\n\nSupported shells: bash, zsh. The command only prints completion scripts and never edits shell config files.";
const DOCTOR_HELP: &str = "Diagnose and optionally fix repository workflow issues.\n\nUsage shortcuts:\n  kanban doctor [REPO_ROOT]        Same as `kanban doctor show [REPO_ROOT]`\n  kanban doctor help               Print this help text\n\nEffects depend on subcommand; `show` is read-only while `fix` rewrites only the affected markdown files.";
const BASH_DATE_PLACEHOLDER: &str = "YYYY-MM-DD";

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CompletionTarget {
    Bash,
    Zsh,
    Help,
}

impl CompletionTarget {
    fn generator(self) -> Option<clap_complete::Shell> {
        match self {
            CompletionTarget::Bash => Some(clap_complete::Shell::Bash),
            CompletionTarget::Zsh => Some(clap_complete::Shell::Zsh),
            CompletionTarget::Help => None,
        }
    }
}

/// Kind of IDs to list for shell completion.
#[derive(Copy, Clone, Debug, ValueEnum)]
enum ListIdsKind {
    Sprints,
    Stories,
    StoriesWithTitles,
    Epics,
}

/// Output format for the `--format` global flag.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Subcommand)]
enum ConfigCommand {
    #[command(
        about = "Show effective kanban configuration. Effect: read-only inspection of `.kanban/*.json`. Side effects: none."
    )]
    Show {
        #[arg(help = "Repository path to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Get one config value. Effect: read-only inspection of `.kanban/*.json`. Side effects: none."
    )]
    Get {
        #[arg(help = "Configuration key, for example paths.backlog or theme.color_mode.")]
        key: String,
        #[arg(help = "Repository path to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Set one config value. Effect: rewrites the owning JSON file in `.kanban/`. Side effects: creates no files outside `.kanban/`."
    )]
    Set {
        #[arg(help = "Configuration key, for example paths.backlog or theme.color_mode.")]
        key: String,
        #[arg(
            help = "Configuration value. Use comma-separated values for story_points.allowed_values."
        )]
        value: String,
        #[arg(help = "Repository path to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum DoctorCommand {
    #[command(
        about = "Diagnose repository workflow issues. Effect: read-only inspection with actionable findings. Side effects: none."
    )]
    Show {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Guide fixes for doctor findings. Effect: rewrites affected markdown files one issue at a time. Side effects: prompts before each fix."
    )]
    Fix {
        #[arg(help = "Optional scope: a story id like US-F1-053 or the literal `current`.")]
        target: Option<String>,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum WebCommand {
    #[command(
        about = "Start the local kanban web UI. Effect: launches tools/kanban-web and writes .kanban/run/web.pid plus .kanban/run/web.log. Side effects: no backlog markdown is modified."
    )]
    Start {
        #[arg(long, help = "Run in the foreground instead of writing a PID file.")]
        foreground: bool,
        #[arg(
            long,
            help = "Open the configured web URL in the default browser after start."
        )]
        open: bool,
        #[arg(long, help = "Run the web server through npm run dev:server.")]
        dev: bool,
        #[arg(
            long,
            help = "Build tools/kanban-web before starting in production mode."
        )]
        build: bool,
        #[arg(help = "Repository root to serve. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Stop the local kanban web UI. Effect: sends SIGTERM to the recorded PID and removes stale runtime files. Side effects: no backlog markdown is modified."
    )]
    Stop {
        #[arg(
            help = "Repository root whose web UI should stop. Defaults to the current directory."
        )]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Restart the local kanban web UI. Effect: stop then start using the supplied start flags. Side effects: updates .kanban/run runtime files only."
    )]
    Restart {
        #[arg(
            long,
            help = "Open the configured web URL in the default browser after restart."
        )]
        open: bool,
        #[arg(long, help = "Run the web server through npm run dev:server.")]
        dev: bool,
        #[arg(
            long,
            help = "Build tools/kanban-web before starting in production mode."
        )]
        build: bool,
        #[arg(help = "Repository root to serve. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Show local kanban web UI process status. Effect: reads .kanban/run/web.pid. Side effects: none."
    )]
    Status {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Print the local kanban web UI log. Effect: reads .kanban/run/web.log. Side effects: none."
    )]
    Log {
        #[arg(long, value_name = "N", help = "Only print the last N log lines.")]
        lines: Option<usize>,
        #[arg(short, long, help = "Follow appended log output until interrupted.")]
        follow: bool,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Initialize `.kanban` in the repository root. Effect: creates default JSON config files in `.kanban/`. Side effects: no backlog files are modified."
    )]
    Init {
        #[arg(help = "Repository path to initialize. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Inspect or change repository-local kanban configuration. Effects depend on subcommand; write subcommands only touch `.kanban/*.json`."
    )]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(
        about = "Inspect and maintain sprint files. Effects depend on subcommand; write subcommands state their markdown side effects."
    )]
    Sprint {
        #[command(subcommand)]
        command: SprintCommand,
    },
    #[command(
        about = "Inspect phase backlog state. Effect: read-only unless a nested command explicitly says otherwise. Side effects: none for current subcommands."
    )]
    Phase {
        #[command(subcommand)]
        command: PhaseCommand,
    },
    #[command(
        about = "Inspect or move user stories. Effects depend on subcommand; write operations mutate canonical story frontmatter and sprint roster markdown."
    )]
    Story {
        #[command(subcommand)]
        command: StoryCommand,
    },
    #[command(
        about = "Maintain sprint task logs. Effect: mutates sibling .tasks.md files only. Side effects: no standalone task artifacts are created."
    )]
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    #[command(
        about = "Control the local kanban web UI process. Effects are limited to .kanban/run runtime files and the local web server process."
    )]
    Web {
        #[command(subcommand)]
        command: WebCommand,
    },
    #[command(
        about = "Generate shell completion scripts. Effect: read-only output to stdout. Side effects: none.",
        long_about = COMPLETION_HELP
    )]
    Completion {
        #[arg(
            help = "Shell to generate completion for, or help for setup instructions. Supported values: bash, zsh, help."
        )]
        target: CompletionTarget,
    },
    #[command(
        about = "Validate repository workflow metadata. Effect: read-only validation of backlog and sprint markdown. Side effects: none."
    )]
    Validate {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Diagnose and optionally fix repository workflow issues. Effects depend on subcommand; `show` is read-only while `fix` rewrites only the affected markdown files.",
        long_about = DOCTOR_HELP,
        override_usage = "kanban doctor [REPO_ROOT]\n       kanban doctor <COMMAND>"
    )]
    Doctor {
        #[command(subcommand)]
        command: DoctorCommand,
    },
    #[command(
        hide = true,
        about = "List IDs for shell completion. Effect: read-only listing of sprint names, story IDs, or epic IDs from the repository. Side effects: none."
    )]
    ListIds {
        #[arg(help = "Kind of IDs to list: sprints, stories, stories-with-titles, or epics.")]
        kind: ListIdsKind,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

fn command_repo_root(command: &Command) -> Option<&PathBuf> {
    match command {
        Command::Init { repo_root }
        | Command::Validate { repo_root }
        | Command::ListIds { repo_root, .. } => Some(repo_root),
        Command::Config { command } => match command {
            ConfigCommand::Show { repo_root }
            | ConfigCommand::Get { repo_root, .. }
            | ConfigCommand::Set { repo_root, .. } => Some(repo_root),
        },
        Command::Sprint { command } => match command {
            SprintCommand::Current { repo_root }
            | SprintCommand::List { repo_root }
            | SprintCommand::Show { repo_root, .. }
            | SprintCommand::Create { repo_root, .. }
            | SprintCommand::Rollover { repo_root, .. }
            | SprintCommand::Sync { repo_root } => Some(repo_root),
        },
        Command::Phase { command } => match command {
            PhaseCommand::Show { repo_root, .. } => Some(repo_root),
        },
        Command::Story { command } => match command {
            StoryCommand::Show { repo_root, .. }
            | StoryCommand::List { repo_root, .. }
            | StoryCommand::Move { repo_root, .. }
            | StoryCommand::Plan { repo_root, .. }
            | StoryCommand::Update { repo_root, .. } => Some(repo_root),
        },
        Command::Task { command } => match command {
            TaskCommand::Add { repo_root, .. } | TaskCommand::Update { repo_root, .. } => {
                Some(repo_root)
            }
        },
        Command::Web { command } => match command {
            WebCommand::Start { repo_root, .. }
            | WebCommand::Stop { repo_root }
            | WebCommand::Restart { repo_root, .. }
            | WebCommand::Status { repo_root }
            | WebCommand::Log { repo_root, .. } => Some(repo_root),
        },
        Command::Doctor { command } => match command {
            DoctorCommand::Show { repo_root } | DoctorCommand::Fix { repo_root, .. } => {
                Some(repo_root)
            }
        },
        Command::Completion { .. } => None,
    }
}

fn theme_for_command(command: &Command) -> Theme {
    let color_mode = command_repo_root(command)
        .and_then(|repo_root| {
            kanban_core::load_kanban_config(repo_root)
                .ok()
                .map(|config| config.theme.color_mode)
        })
        .unwrap_or(ColorMode::Auto);
    Theme::for_stdout(color_mode)
}

/// Serialize a `JsonEnvelope` to stdout and return its exit code.
fn print_envelope<T: Serialize>(env: &JsonEnvelope<T>) -> i32 {
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

fn invalid_argument_envelope<T: Serialize>(kind: &'static str, message: impl Into<String>) -> i32 {
    print_envelope(&JsonEnvelope::<T>::error(
        kind,
        KanbanErrorBody::new(KanbanErrorCode::InvalidArgument, message),
    ))
}

fn completion_target_label(target: CompletionTarget) -> &'static str {
    match target {
        CompletionTarget::Bash => "bash",
        CompletionTarget::Zsh => "zsh",
        CompletionTarget::Help => "help",
    }
}

fn list_ids_kind_label(kind: ListIdsKind) -> &'static str {
    match kind {
        ListIdsKind::Sprints => "sprints",
        ListIdsKind::Stories => "stories",
        ListIdsKind::StoriesWithTitles => "stories-with-titles",
        ListIdsKind::Epics => "epics",
    }
}

fn completion_output(target: CompletionTarget) -> CompletionDto {
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

fn json_story_frontmatter_updates(
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

fn forward_slashed_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[derive(Debug, Clone, Serialize)]
struct WebStatusDto {
    state: String,
    pid: Option<u32>,
    stale_pid: Option<u32>,
    url: String,
    pid_file: String,
    log_file: String,
}

#[derive(Debug, Clone, Serialize)]
struct WebStartDto {
    state: String,
    pid: u32,
    url: String,
    requested_port: u16,
    actual_port: u16,
    port_changed: bool,
    log_file: String,
}

#[derive(Debug, Clone, Serialize)]
struct WebStopDto {
    stopped: bool,
    before: WebStatusDto,
    after: WebStatusDto,
}

#[derive(Debug, Clone, Serialize)]
struct WebRestartDto {
    stopped_existing: bool,
    started: WebStartDto,
}

#[derive(Debug, Clone, Serialize)]
struct WebLogDto {
    exists: bool,
    path: String,
    line_count: usize,
    lines: Vec<String>,
    content: String,
}

fn web_status_json(repo_root: &Path) -> Result<WebStatusDto> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    let process_state = read_web_process_state(&paths)?;
    let status_port = match process_state {
        WebProcessState::Running(_) => read_web_port_file(&paths).unwrap_or(config.web.port),
        WebProcessState::Stopped | WebProcessState::Stale(_) => config.web.port,
    };
    let url = format!("http://{}:{}", config.web.host, status_port);
    let (state, pid, stale_pid) = match process_state {
        WebProcessState::Stopped => ("stopped".to_string(), None, None),
        WebProcessState::Running(pid) => ("running".to_string(), Some(pid), None),
        WebProcessState::Stale(pid) => ("stale".to_string(), None, pid),
    };
    Ok(WebStatusDto {
        state,
        pid,
        stale_pid,
        url,
        pid_file: forward_slashed_path(&paths.pid_file),
        log_file: forward_slashed_path(&paths.log_file),
    })
}

fn web_start_json(repo_root: &Path, open: bool, dev: bool) -> Result<WebStartDto> {
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root;
    let paths = web_runtime_paths(&repo_root);
    fs::create_dir_all(&paths.run_dir)
        .with_context(|| format!("create web runtime directory {}", paths.run_dir.display()))?;

    match read_web_process_state(&paths)? {
        WebProcessState::Running(pid) => bail!("kanban web is already running with PID {pid}."),
        WebProcessState::Stale(_) => remove_pid_file(&paths)?,
        WebProcessState::Stopped => {}
    }

    if !dev && !web_production_entry(&repo_root).is_file() {
        bail!(
            "built web server not found at {}. Run `kanban web start --build` or use `kanban web start --dev`.",
            web_production_entry(&repo_root).display()
        );
    }

    let port = resolve_web_port(&config.web.host, config.web.port)?;
    let url = format!("http://{}:{}", config.web.host, port.actual);
    let spec = build_web_start_command_spec(&repo_root, dev);
    write_web_port_file(&paths, port.actual)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
        .with_context(|| format!("open web log {}", paths.log_file.display()))?;
    let stderr = log
        .try_clone()
        .with_context(|| format!("clone web log handle {}", paths.log_file.display()))?;
    let mut command = process_from_spec(&spec);
    command
        .env("KANBAN_WEB_PORT", port.actual.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    command.process_group(0);
    let child = command
        .spawn()
        .with_context(|| format!("start web server with {}", spec.program))?;
    fs::write(&paths.pid_file, format!("{}\n", child.id()))
        .with_context(|| format!("write PID file {}", paths.pid_file.display()))?;

    if open {
        open_browser_url(&url)?;
    }

    Ok(WebStartDto {
        state: "running".to_string(),
        pid: child.id(),
        url,
        requested_port: port.requested,
        actual_port: port.actual,
        port_changed: port.changed(),
        log_file: forward_slashed_path(&paths.log_file),
    })
}

fn web_stop_json(repo_root: &Path) -> Result<WebStopDto> {
    let before = web_status_json(repo_root)?;
    let stopped = stop_web(&Theme::for_stdout(ColorMode::Never), repo_root, true)?;
    let after = web_status_json(repo_root)?;
    Ok(WebStopDto {
        stopped,
        before,
        after,
    })
}

fn web_log_json(repo_root: &Path, lines: Option<usize>) -> Result<WebLogDto> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    if !paths.log_file.exists() {
        return Ok(WebLogDto {
            exists: false,
            path: forward_slashed_path(&paths.log_file),
            line_count: 0,
            lines: Vec::new(),
            content: String::new(),
        });
    }

    let content = fs::read_to_string(&paths.log_file)
        .with_context(|| format!("read web log {}", paths.log_file.display()))?;
    let selected_lines = match lines {
        Some(0) => Vec::new(),
        Some(limit) => content
            .lines()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(str::to_string)
            .collect(),
        None => content.lines().map(str::to_string).collect(),
    };
    let selected_content = selected_lines.join("\n");
    let line_count = selected_lines.len();
    Ok(WebLogDto {
        exists: true,
        path: forward_slashed_path(&paths.log_file),
        line_count,
        lines: selected_lines,
        content: selected_content,
    })
}

/// Dispatch the JSON output path for a supported command.
fn emit_json(command: &Command) -> i32 {
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
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebRuntimePaths {
    run_dir: PathBuf,
    pid_file: PathBuf,
    port_file: PathBuf,
    log_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebStartCommandSpec {
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebPortResolution {
    requested: u16,
    actual: u16,
}

impl WebPortResolution {
    fn changed(&self) -> bool {
        self.requested != self.actual
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WebProcessState {
    Stopped,
    Running(u32),
    Stale(Option<u32>),
}

fn web_runtime_paths(repo_root: &Path) -> WebRuntimePaths {
    let run_dir = repo_root.join(".kanban/run");
    WebRuntimePaths {
        pid_file: run_dir.join("web.pid"),
        port_file: run_dir.join("web.port"),
        log_file: run_dir.join("web.log"),
        run_dir,
    }
}

fn web_app_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tools/kanban-web")
}

fn web_production_entry(repo_root: &Path) -> PathBuf {
    web_app_dir(repo_root).join("dist/server/index.js")
}

fn build_web_start_command_spec(repo_root: &Path, dev: bool) -> WebStartCommandSpec {
    let web_dir = web_app_dir(repo_root);
    if dev {
        WebStartCommandSpec {
            program: "npm".to_string(),
            args: vec![
                "--prefix".to_string(),
                web_dir.to_string_lossy().into_owned(),
                "run".to_string(),
                "dev:server".to_string(),
            ],
            cwd: repo_root.to_path_buf(),
        }
    } else {
        WebStartCommandSpec {
            program: "node".to_string(),
            args: vec![
                web_production_entry(repo_root)
                    .to_string_lossy()
                    .into_owned(),
            ],
            cwd: repo_root.to_path_buf(),
        }
    }
}

fn build_web_build_command_spec(repo_root: &Path) -> WebStartCommandSpec {
    WebStartCommandSpec {
        program: "npm".to_string(),
        args: vec![
            "--prefix".to_string(),
            web_app_dir(repo_root).to_string_lossy().into_owned(),
            "run".to_string(),
            "build".to_string(),
        ],
        cwd: repo_root.to_path_buf(),
    }
}

fn process_from_spec(spec: &WebStartCommandSpec) -> ProcessCommand {
    let mut command = ProcessCommand::new(&spec.program);
    command.args(&spec.args).current_dir(&spec.cwd);
    command
}

fn resolve_web_port(host: &str, requested: u16) -> Result<WebPortResolution> {
    for port in requested..=u16::MAX {
        match TcpListener::bind((host, port)) {
            Ok(listener) => {
                drop(listener);
                return Ok(WebPortResolution {
                    requested,
                    actual: port,
                });
            }
            Err(error) if error.kind() == ErrorKind::AddrInUse => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("check whether {host}:{port} is available"));
            }
        }
    }

    bail!("No available port found at or above {requested} on {host}.")
}

fn read_web_process_state(paths: &WebRuntimePaths) -> Result<WebProcessState> {
    if !paths.pid_file.exists() {
        return Ok(WebProcessState::Stopped);
    }

    let raw = fs::read_to_string(&paths.pid_file)
        .with_context(|| format!("read web PID file {}", paths.pid_file.display()))?;
    let trimmed = raw.trim();
    let Ok(pid) = trimmed.parse::<u32>() else {
        return Ok(WebProcessState::Stale(None));
    };
    if pid == 0 {
        return Ok(WebProcessState::Stale(None));
    }

    if process_exists(pid) {
        Ok(WebProcessState::Running(pid))
    } else {
        Ok(WebProcessState::Stale(Some(pid)))
    }
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn process_exists(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<()> {
    let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if result == 0 || !process_exists(pid) {
        Ok(())
    } else {
        bail!("failed to stop web process {pid}");
    }
}

#[cfg(not(unix))]
fn terminate_process(_pid: u32) -> Result<()> {
    bail!("kanban web stop is only implemented on Unix-like systems.")
}

fn remove_pid_file(paths: &WebRuntimePaths) -> Result<()> {
    if paths.pid_file.exists() {
        fs::remove_file(&paths.pid_file)
            .with_context(|| format!("remove PID file {}", paths.pid_file.display()))?;
    }
    if paths.port_file.exists() {
        fs::remove_file(&paths.port_file)
            .with_context(|| format!("remove web port file {}", paths.port_file.display()))?;
    }
    Ok(())
}

fn read_web_port_file(paths: &WebRuntimePaths) -> Option<u16> {
    fs::read_to_string(&paths.port_file)
        .ok()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .filter(|port| *port != 0)
}

fn write_web_port_file(paths: &WebRuntimePaths, port: u16) -> Result<()> {
    fs::write(&paths.port_file, format!("{port}\n"))
        .with_context(|| format!("write web port file {}", paths.port_file.display()))
}

fn run_web_build(repo_root: &Path) -> Result<()> {
    let spec = build_web_build_command_spec(repo_root);
    let status = process_from_spec(&spec)
        .status()
        .with_context(|| format!("run {} {}", spec.program, spec.args.join(" ")))?;
    if !status.success() {
        bail!("web build failed with status {status}");
    }
    Ok(())
}

fn open_browser_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = ProcessCommand::new("open");
    #[cfg(target_os = "linux")]
    let mut command = ProcessCommand::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = ProcessCommand::new("cmd");
        command.arg("/C").arg("start");
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        bail!("opening a browser is not supported on this platform");
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        command.arg(url);
        let status = command
            .status()
            .with_context(|| format!("open browser URL {url}"))?;
        if !status.success() {
            bail!("open browser command failed with status {status}");
        }
        Ok(())
    }
}

fn start_web(
    theme: &Theme,
    repo_root: &Path,
    foreground: bool,
    open: bool,
    dev: bool,
    build: bool,
) -> Result<()> {
    if dev && build {
        bail!("--build cannot be combined with --dev.");
    }
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root;
    let paths = web_runtime_paths(&repo_root);
    fs::create_dir_all(&paths.run_dir)
        .with_context(|| format!("create web runtime directory {}", paths.run_dir.display()))?;

    match read_web_process_state(&paths)? {
        WebProcessState::Running(pid) => {
            eprint!(
                "{}",
                render_web_already_running_error(
                    theme,
                    pid,
                    detected_terminal_width().unwrap_or(DEFAULT_OUTPUT_WIDTH)
                )
            );
            std::process::exit(1);
        }
        WebProcessState::Stale(_) => remove_pid_file(&paths)?,
        WebProcessState::Stopped => {}
    }

    if build {
        println!("{}", theme.label("Building kanban web UI..."));
        run_web_build(&repo_root)?;
    }
    if !dev && !web_production_entry(&repo_root).is_file() {
        bail!(
            "built web server not found at {}. Run `kanban web start --build` or use `kanban web start --dev`.",
            web_production_entry(&repo_root).display()
        );
    }

    let port = resolve_web_port(&config.web.host, config.web.port)?;
    if port.changed() {
        println!(
            "{}",
            render_web_port_fallback_warning(theme, &config.web.host, port.requested, port.actual)
        );
    }

    let url = format!("http://{}:{}", config.web.host, port.actual);
    let spec = build_web_start_command_spec(&repo_root, dev);
    if foreground {
        println!("{} {url}", theme.success("Starting kanban web UI:"));
        if open && let Err(error) = open_browser_url(&url) {
            eprintln!("{} {error}", theme.warning("Could not open browser:"));
        }
        let status = process_from_spec(&spec)
            .env("KANBAN_WEB_PORT", port.actual.to_string())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("start web server with {}", spec.program))?;
        if !status.success() {
            bail!("web server exited with status {status}");
        }
        return Ok(());
    }

    write_web_port_file(&paths, port.actual)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
        .with_context(|| format!("open web log {}", paths.log_file.display()))?;
    let stderr = log
        .try_clone()
        .with_context(|| format!("clone web log handle {}", paths.log_file.display()))?;
    let mut command = process_from_spec(&spec);
    command
        .env("KANBAN_WEB_PORT", port.actual.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    command.process_group(0);
    let child = command
        .spawn()
        .with_context(|| format!("start web server with {}", spec.program))?;
    fs::write(&paths.pid_file, format!("{}\n", child.id()))
        .with_context(|| format!("write PID file {}", paths.pid_file.display()))?;

    println!("{} {url}", theme.success("Started kanban web UI:"));
    println!("{} {}", theme.label("PID:"), child.id());
    println!(
        "{} {}",
        theme.label("Log:"),
        theme.path(paths.log_file.display())
    );
    if open && let Err(error) = open_browser_url(&url) {
        eprintln!("{} {error}", theme.warning("Could not open browser:"));
    }
    Ok(())
}

fn stop_web(theme: &Theme, repo_root: &Path, quiet: bool) -> Result<bool> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    match read_web_process_state(&paths)? {
        WebProcessState::Stopped => {
            if !quiet {
                println!("{}", theme.warning("kanban web UI is not running."));
            }
            Ok(false)
        }
        WebProcessState::Stale(pid) => {
            remove_pid_file(&paths)?;
            if !quiet {
                match pid {
                    Some(pid) => println!("{} stale PID {pid}", theme.warning("Removed")),
                    None => println!("{}", theme.warning("Removed stale web PID file.")),
                }
            }
            Ok(false)
        }
        WebProcessState::Running(pid) => {
            terminate_process(pid)?;
            for _ in 0..30 {
                if !process_exists(pid) {
                    remove_pid_file(&paths)?;
                    if !quiet {
                        println!("{} PID {pid}", theme.success("Stopped kanban web UI:"));
                    }
                    return Ok(true);
                }
                thread::sleep(Duration::from_millis(100));
            }
            bail!("web process {pid} did not stop after SIGTERM");
        }
    }
}

fn print_web_status(theme: &Theme, repo_root: &Path) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    let process_state = read_web_process_state(&paths)?;
    let status_port = match process_state {
        WebProcessState::Running(_) => read_web_port_file(&paths).unwrap_or(config.web.port),
        WebProcessState::Stopped | WebProcessState::Stale(_) => config.web.port,
    };
    let url = format!("http://{}:{}", config.web.host, status_port);
    match process_state {
        WebProcessState::Running(pid) => {
            println!("{} running", theme.success("kanban web UI:"));
            println!("{} {pid}", theme.label("PID:"));
            println!("{} {url}", theme.label("URL:"));
            println!(
                "{} {}",
                theme.label("Log:"),
                theme.path(paths.log_file.display())
            );
        }
        WebProcessState::Stopped => {
            println!("{} stopped", theme.warning("kanban web UI:"));
            println!("{} {url}", theme.label("URL:"));
        }
        WebProcessState::Stale(pid) => {
            match pid {
                Some(pid) => println!("{} stale PID {pid}", theme.warning("kanban web UI:")),
                None => println!("{} stale PID file", theme.warning("kanban web UI:")),
            }
            println!(
                "{} {}",
                theme.label("PID file:"),
                theme.path(paths.pid_file.display())
            );
        }
    }
    Ok(())
}

fn render_web_already_running_error(theme: &Theme, pid: u32, width: usize) -> String {
    let icon = "✖";
    let prefix_width = display_width(icon) + 1;
    let content_width = width.saturating_sub(prefix_width).max(1);
    let mut output = String::new();
    let primary = format!("Error: kanban web is already running with PID {pid}.");
    let guidance = [
        InlineToken::plain("Use", false),
        InlineToken::command("`kanban web status`", true),
        InlineToken::plain("or", true),
        InlineToken::command("`kanban web restart`", true),
        InlineToken::plain(".", false),
    ];

    for (index, line) in wrap_text(&primary, content_width).iter().enumerate() {
        if index == 0 {
            if let Some(rest) = line.strip_prefix("Error:") {
                push_line(
                    &mut output,
                    &format!("{} {}{}", theme.error(icon), theme.error("Error:"), rest),
                );
            } else {
                push_line(&mut output, &format!("{} {line}", theme.error(icon)));
            }
        } else {
            push_line(&mut output, &format!("{}{line}", " ".repeat(prefix_width)));
        }
    }
    push_wrapped_inline_message(&mut output, theme, prefix_width, content_width, &guidance);

    output
}

fn render_web_port_fallback_warning(
    theme: &Theme,
    host: &str,
    requested_port: u16,
    actual_port: u16,
) -> String {
    format!(
        "{} another service is already using http://{}:{}; starting kanban web UI on http://{}:{} instead.",
        theme.warning("Warning:"),
        host,
        requested_port,
        host,
        actual_port
    )
}

#[derive(Copy, Clone)]
enum InlineStyle {
    Plain,
    Command,
}

struct InlineToken {
    text: &'static str,
    style: InlineStyle,
    leading_space: bool,
}

impl InlineToken {
    const fn plain(text: &'static str, leading_space: bool) -> Self {
        Self {
            text,
            style: InlineStyle::Plain,
            leading_space,
        }
    }

    const fn command(text: &'static str, leading_space: bool) -> Self {
        Self {
            text,
            style: InlineStyle::Command,
            leading_space,
        }
    }
}

fn push_wrapped_inline_message(
    output: &mut String,
    theme: &Theme,
    indent: usize,
    width: usize,
    tokens: &[InlineToken],
) {
    let mut lines: Vec<Vec<(InlineStyle, String)>> = Vec::new();
    let mut current: Vec<(InlineStyle, String)> = Vec::new();
    let mut current_width = 0;

    for token in tokens {
        let token_width = display_width(token.text);
        let space_width = usize::from(token.leading_space && !current.is_empty());
        if !current.is_empty() && current_width + space_width + token_width > width {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }

        if token.leading_space && !current.is_empty() {
            current.push((InlineStyle::Plain, " ".to_string()));
            current_width += 1;
        }
        current.push((token.style, token.text.to_string()));
        current_width += token_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    for line in lines {
        let mut rendered = " ".repeat(indent);
        for (style, text) in line {
            match style {
                InlineStyle::Plain => rendered.push_str(&text),
                InlineStyle::Command => rendered.push_str(&theme.command(text)),
            }
        }
        push_line(output, &rendered);
    }
}

fn print_log_tail(content: &str, lines: Option<usize>) {
    match lines {
        Some(0) => {}
        Some(limit) => {
            let selected = content.lines().rev().take(limit).collect::<Vec<_>>();
            for line in selected.iter().rev() {
                println!("{line}");
            }
        }
        None => print!("{content}"),
    }
}

fn print_web_log(
    theme: &Theme,
    repo_root: &Path,
    lines: Option<usize>,
    follow: bool,
) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    let paths = web_runtime_paths(&config.repo_root);
    if !paths.log_file.exists() {
        println!(
            "{} {}",
            theme.warning("No web log found:"),
            theme.path(paths.log_file.display())
        );
        return Ok(());
    }

    let content = fs::read_to_string(&paths.log_file)
        .with_context(|| format!("read web log {}", paths.log_file.display()))?;
    print_log_tail(&content, lines);
    if !follow {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .read(true)
        .open(&paths.log_file)
        .with_context(|| format!("open web log {}", paths.log_file.display()))?;
    file.seek(SeekFrom::End(0))?;
    loop {
        let mut appended = String::new();
        file.read_to_string(&mut appended)?;
        if !appended.is_empty() {
            print!("{appended}");
            std::io::stdout().flush()?;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn print_sprint_overview(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) {
    print!("{}", render_sprint_overview(theme, layout, sprint));
}

fn print_sprint_overview_short(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) {
    print!("{}", render_sprint_overview_short(theme, layout, sprint));
}

fn render_sprint_overview(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) -> String {
    let mut output = String::new();
    let content_width = sprint_content_width(layout.width);
    let story_table_width = sprint_story_table_width(layout.width);
    let blocked_table_width = sprint_table_width(layout.width);
    let mut has_content_section = false;

    // Dashboard header band: top separator, progress line, count line, bottom separator
    push_sprint_header_band(&mut output, theme, layout, sprint);

    // Sprint goal (below bottom separator)
    if let Some(goal) = &sprint.sprint_goal {
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        push_wrapped_label_value_inset(&mut output, theme, "Sprint Goal:", goal, content_width);
    }

    // Warnings
    if !sprint.warnings.is_empty() {
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        for warning in &sprint.warnings {
            push_wrapped_hanging_line_inset(&mut output, "", warning, content_width, |v| {
                theme.warning(v)
            });
        }
    }

    // Status sections expanded with story rows.
    for status in ["todo", "in-progress", "ready-for-qa", "done"] {
        let stories = sprint
            .stories_by_status
            .get(status)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        push_sprint_section_divider_before_next(
            &mut output,
            theme,
            layout.width,
            &mut has_content_section,
        );
        let icon_label = theme.status_text(status, format!("{} {status}", status_icon(status)));
        let status_points = sum_story_points(stories.iter());
        let points_label = theme.story_points(format_story_points(status_points));
        let story_count = format_story_count(stories.len());
        if stories.is_empty() {
            push_inset_line(
                &mut output,
                &format!(
                    "{icon_label}   {}   {points_label}   · none",
                    theme.count(story_count)
                ),
            );
        } else {
            push_inset_line(
                &mut output,
                &format!(
                    "{icon_label}   {}   {points_label}",
                    theme.count(story_count)
                ),
            );
            let points_width =
                story_points_column_width(sprint.stories_by_status.values().flat_map(|v| v.iter()));
            push_story_table(&mut output, theme, story_table_width, stories, points_width);
        }
    }

    // Summary footer: ✗ blocked N
    let blocked_count = sprint
        .stories_by_status
        .get("blocked")
        .map(|v| v.len())
        .unwrap_or(0);
    let blocked_points = sprint
        .stories_by_status
        .get("blocked")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);
    push_sprint_section_divider_before_next(
        &mut output,
        theme,
        layout.width,
        &mut has_content_section,
    );
    let blocked_style = if blocked_count > 0 {
        Style::Red
    } else {
        Style::Muted
    };
    let blocked_part = theme.paint(blocked_style, format!("{} blocked", status_icon("blocked")));
    push_inset_line(
        &mut output,
        &format!(
            "{}   {}   {}",
            blocked_part,
            theme.count(format_story_count(blocked_count)),
            theme.story_points(format_story_points(blocked_points)),
        ),
    );

    // Blocked work detail callout
    push_sprint_section_divider_before_next(
        &mut output,
        theme,
        layout.width,
        &mut has_content_section,
    );
    push_inset_line(&mut output, &theme.heading("Blocked work"));
    if sprint.blocked_work.is_empty() {
        push_inset_line(&mut output, "- none");
    } else {
        push_blocked_work_table(
            &mut output,
            theme,
            blocked_table_width,
            &sprint.blocked_work,
        );
    }

    output
}

fn render_sprint_overview_short(
    theme: &Theme,
    layout: OutputLayout,
    sprint: &SprintOverview,
) -> String {
    let mut output = String::new();
    push_sprint_header_band(&mut output, theme, layout, sprint);
    output
}

fn title_case_headline(headline: &str) -> String {
    headline
        .split([' ', '-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_story_count(count: usize) -> String {
    if count == 1 {
        "1 story".to_string()
    } else {
        format!("{count} stories")
    }
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_inset_line(output: &mut String, line: &str) {
    push_line(
        output,
        &format!("{}{}", " ".repeat(SPRINT_CONTENT_INSET), line),
    );
}

fn sprint_content_width(width: usize) -> usize {
    width.saturating_sub(SPRINT_CONTENT_INSET * 2).max(1)
}

fn sprint_table_width(width: usize) -> usize {
    width.saturating_sub(SPRINT_CONTENT_INSET).max(1)
}

fn sprint_story_table_width(width: usize) -> usize {
    width
        .saturating_sub(display_width(SPRINT_STORY_ROW_PREFIX))
        .max(1)
}

fn push_sprint_section_divider(output: &mut String, theme: &Theme, width: usize) {
    push_line(output, &theme.paint(Style::Muted, "─".repeat(width)));
}

fn push_sprint_section_divider_before_next(
    output: &mut String,
    theme: &Theme,
    width: usize,
    has_content_section: &mut bool,
) {
    if *has_content_section {
        push_sprint_section_divider(output, theme, width);
    }
    *has_content_section = true;
}

fn push_wrapped_label_value_inset(
    output: &mut String,
    theme: &Theme,
    label: &str,
    value: &str,
    width: usize,
) {
    let prefix_width = display_width(label) + 1;
    let value_width = width.saturating_sub(prefix_width).max(1);
    let wrapped = wrap_text(value, value_width);
    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            push_inset_line(output, &format!("{} {line}", theme.label(label)));
        } else {
            push_inset_line(output, &format!("{}{line}", " ".repeat(prefix_width)));
        }
    }
}

fn push_wrapped_hanging_line_inset(
    output: &mut String,
    prefix: &str,
    value: &str,
    width: usize,
    style: impl Fn(&str) -> String,
) {
    let value_width = width.saturating_sub(display_width(prefix)).max(1);
    let wrapped = wrap_text(value, value_width);
    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            push_inset_line(output, &format!("{prefix}{}", style(line)));
        } else {
            push_inset_line(
                output,
                &format!("{}{line}", " ".repeat(display_width(prefix))),
            );
        }
    }
}

fn push_wrapped_hanging_line(
    output: &mut String,
    prefix: &str,
    value: &str,
    width: usize,
    style: impl Fn(&str) -> String,
) {
    let value_width = width.saturating_sub(display_width(prefix)).max(1);
    let wrapped = wrap_text(value, value_width);
    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            push_line(output, &format!("{prefix}{}", style(line)));
        } else {
            push_line(
                output,
                &format!("{}{line}", " ".repeat(display_width(prefix))),
            );
        }
    }
}

#[derive(Copy, Clone)]
enum CellStyle {
    Id,
    Warning,
    Precolored,
}

struct TableCell {
    text: String,
    style: Option<CellStyle>,
    preformatted: bool,
}

struct DynamicTableColumn {
    title: String,
    width: usize,
}

impl TableCell {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            preformatted: false,
        }
    }

    fn styled(text: impl Into<String>, style: CellStyle) -> Self {
        Self {
            text: text.into(),
            style: Some(style),
            preformatted: false,
        }
    }

    fn preformatted(text: impl Into<String>, style: CellStyle) -> Self {
        Self {
            text: text.into(),
            style: Some(style),
            preformatted: true,
        }
    }
}

fn push_story_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    stories: &[StoryOverview],
    points_width: usize,
) {
    let columns = story_table_columns(width, stories, points_width);
    let rows = stories
        .iter()
        .map(|story| {
            vec![
                TableCell::preformatted(
                    format_colored_story_status_label(theme, story, points_width),
                    CellStyle::Precolored,
                ),
                TableCell::new(&story.title),
                TableCell::new(extract_assignee_name(&story.assignee)),
                TableCell::styled(
                    format_colored_task_summary(theme, story.task_summary.as_ref()),
                    CellStyle::Precolored,
                ),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_story_rows(output, theme, &columns, &rows);
}

fn push_phase_story_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    stories: &[&StoryOverview],
    points_width: usize,
) {
    let columns = phase_story_table_columns(width, stories, points_width);
    let rows = stories
        .iter()
        .map(|story| {
            vec![
                TableCell::preformatted(
                    format_colored_story_status_label(theme, story, points_width),
                    CellStyle::Precolored,
                ),
                TableCell::new(&story.title),
                TableCell::new(story.sprint.as_deref().unwrap_or("~")),
                TableCell::new(extract_assignee_name(&story.assignee)),
                TableCell::styled(
                    format_colored_task_summary(theme, story.task_summary.as_ref()),
                    CellStyle::Precolored,
                ),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_rows(output, theme, &columns, &rows);
}

fn push_blocked_work_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    items: &[kanban_core::BlockedWorkItem],
) {
    let columns = blocked_work_table_columns(width, items);
    let rows = items
        .iter()
        .map(|item| {
            vec![
                TableCell::styled(&item.story_id, CellStyle::Id),
                TableCell::new(&item.story_title),
                TableCell::styled(
                    item.task_id.clone().unwrap_or_else(|| "-".to_string()),
                    CellStyle::Warning,
                ),
                TableCell::new(item.task_title.clone().unwrap_or_else(|| "-".to_string())),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_rows(output, theme, &columns, &rows);
}

fn story_table_columns(
    width: usize,
    stories: &[StoryOverview],
    points_width: usize,
) -> Vec<(&'static str, usize)> {
    let available = row_content_width(width, 4);
    let id_width = stories
        .iter()
        .map(|story| display_width(&format_story_status_label(story, points_width)))
        .max()
        .unwrap_or(5)
        .clamp(5, 18);
    let task_width = stories
        .iter()
        .map(|story| display_width(&format_compact_task_summary(story.task_summary.as_ref())))
        .max()
        .unwrap_or(5)
        .clamp(5, 17);
    let raw_assignee_width = stories
        .iter()
        .map(|story| display_width(extract_assignee_name(&story.assignee)))
        .max()
        .unwrap_or(8)
        .max(8);
    // Clamp assignee so title always gets at least 20 columns.
    let max_assignee = available.saturating_sub(id_width + task_width + 20);
    let assignee_width = raw_assignee_width.min(max_assignee.max(8));
    let title_width = available
        .saturating_sub(id_width + assignee_width + task_width)
        .max(1);

    vec![
        ("Story", id_width),
        ("Description", title_width),
        ("Assignee", assignee_width),
        ("Tasks", task_width),
    ]
}

fn phase_story_table_columns(
    width: usize,
    stories: &[&StoryOverview],
    points_width: usize,
) -> Vec<(&'static str, usize)> {
    let available = row_content_width(width, 5);
    let id_width = stories
        .iter()
        .map(|story| display_width(&format_story_status_label(story, points_width)))
        .max()
        .unwrap_or(5)
        .clamp(5, 18);
    let sprint_width = stories
        .iter()
        .map(|story| display_width(story.sprint.as_deref().unwrap_or("~")))
        .max()
        .unwrap_or(1)
        .clamp(1, 22);
    let task_width = stories
        .iter()
        .map(|story| display_width(&format_compact_task_summary(story.task_summary.as_ref())))
        .max()
        .unwrap_or(5)
        .clamp(5, 17);
    let raw_assignee_width = stories
        .iter()
        .map(|story| display_width(extract_assignee_name(&story.assignee)))
        .max()
        .unwrap_or(8)
        .max(8);
    let max_assignee = available.saturating_sub(id_width + sprint_width + task_width + 20);
    let assignee_width = raw_assignee_width.min(max_assignee.max(8));
    let title_width = available
        .saturating_sub(id_width + sprint_width + assignee_width + task_width)
        .max(1);

    vec![
        ("Story", id_width),
        ("Description", title_width),
        ("Sprint", sprint_width),
        ("Assignee", assignee_width),
        ("Tasks", task_width),
    ]
}

fn blocked_work_table_columns(
    width: usize,
    items: &[kanban_core::BlockedWorkItem],
) -> Vec<(&'static str, usize)> {
    let available = row_content_width(width, 4);
    let story_width = items
        .iter()
        .map(|item| display_width(&item.story_id))
        .max()
        .unwrap_or(5)
        .clamp(5, 12);
    let task_width = items
        .iter()
        .filter_map(|item| item.task_id.as_deref())
        .map(display_width)
        .max()
        .unwrap_or(4)
        .clamp(4, 10);
    let remaining = available.saturating_sub(story_width + task_width);
    let story_title_width = remaining / 2;
    let task_title_width = remaining.saturating_sub(story_title_width);

    vec![
        ("Story", story_width),
        ("Description", story_title_width.max(16)),
        ("Task", task_width),
        ("Task description", task_title_width.max(16)),
    ]
}

fn row_content_width(width: usize, column_count: usize) -> usize {
    let indent = 2;
    let gaps = column_count.saturating_sub(1) * 2;
    width.saturating_sub(indent + gaps)
}

fn push_wrapped_rows(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    rows: &[Vec<TableCell>],
) {
    for row in rows {
        push_wrapped_table_row(output, theme, columns, row);
    }
}

fn push_wrapped_story_rows(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    rows: &[Vec<TableCell>],
) {
    for row in rows {
        push_wrapped_story_table_row(output, theme, columns, row);
    }
}

fn push_wrapped_table(
    output: &mut String,
    theme: &Theme,
    columns: &[DynamicTableColumn],
    rows: &[Vec<TableCell>],
) {
    let header = columns
        .iter()
        .map(|column| TableCell::preformatted(theme.label(&column.title), CellStyle::Precolored))
        .collect::<Vec<_>>();
    push_wrapped_dynamic_table_row(output, theme, columns, &header);

    let mut separator = String::from("  ");
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            separator.push_str("  ");
        }
        separator.push_str(&theme.paint(Style::Muted, "─".repeat(column.width)));
    }
    push_line(output, &separator);

    for row in rows {
        push_wrapped_dynamic_table_row(output, theme, columns, row);
    }
}

fn push_wrapped_dynamic_table_row(
    output: &mut String,
    theme: &Theme,
    columns: &[DynamicTableColumn],
    row: &[TableCell],
) {
    let wrapped_cells = row
        .iter()
        .zip(columns)
        .map(|(cell, column)| {
            if cell.preformatted {
                vec![cell.text.clone()]
            } else {
                wrap_text(&cell.text, column.width)
            }
        })
        .collect::<Vec<_>>();
    let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);

    for line_index in 0..row_height {
        let mut line = String::new();
        line.push_str("  ");
        for ((cell, column), wrapped) in row.iter().zip(columns).zip(&wrapped_cells) {
            let value = wrapped.get(line_index).map(String::as_str).unwrap_or("");
            let padded = pad_to_width(value, column.width);
            if line.len() > 2 {
                line.push_str("  ");
            }
            line.push_str(&style_table_cell(theme, cell.style, &padded));
        }
        push_line(output, &line);
    }
}

fn push_wrapped_table_row(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    row: &[TableCell],
) {
    let wrapped_cells = row
        .iter()
        .zip(columns)
        .map(|(cell, (_, width))| {
            if cell.preformatted {
                vec![cell.text.clone()]
            } else {
                wrap_text(&cell.text, *width)
            }
        })
        .collect::<Vec<_>>();
    let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);

    for line_index in 0..row_height {
        let mut line = String::new();
        line.push_str("  ");
        for ((cell, (_, width)), wrapped) in row.iter().zip(columns).zip(&wrapped_cells) {
            let value = wrapped.get(line_index).map(String::as_str).unwrap_or("");
            let padded = pad_to_width(value, *width);
            if line.len() > 2 {
                line.push_str("  ");
            }
            line.push_str(&style_table_cell(theme, cell.style, &padded));
        }
        push_line(output, &line);
    }
}

fn push_wrapped_story_table_row(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    row: &[TableCell],
) {
    let wrapped_cells = row
        .iter()
        .zip(columns)
        .map(|(cell, (_, width))| {
            if cell.preformatted {
                vec![cell.text.clone()]
            } else {
                wrap_text(&cell.text, *width)
            }
        })
        .collect::<Vec<_>>();
    let row_height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);

    for line_index in 0..row_height {
        let prefix = if line_index == 0 {
            SPRINT_STORY_ROW_PREFIX.to_string()
        } else {
            " ".repeat(display_width(SPRINT_STORY_ROW_PREFIX))
        };
        let mut line = prefix;
        let prefix_width = display_width(&line);
        for ((cell, (_, width)), wrapped) in row.iter().zip(columns).zip(&wrapped_cells) {
            let value = wrapped.get(line_index).map(String::as_str).unwrap_or("");
            let padded = pad_to_width(value, *width);
            if display_width(&line) > prefix_width {
                line.push_str("  ");
            }
            line.push_str(&style_table_cell(theme, cell.style, &padded));
        }
        push_line(output, &line);
    }
}

fn style_table_cell(theme: &Theme, style: Option<CellStyle>, value: &str) -> String {
    match style {
        Some(CellStyle::Id) => theme.id(value),
        Some(CellStyle::Warning) => theme.warning(value),
        Some(CellStyle::Precolored) => value.to_string(),
        None => value.to_string(),
    }
}

fn pad_to_width(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(display_width(value));
    format!("{value}{}", " ".repeat(padding))
}

fn display_width(value: &str) -> usize {
    let mut count = 0;
    let mut in_escape = false;
    for ch in value.chars() {
        match ch {
            '\x1b' => in_escape = true,
            'm' if in_escape => in_escape = false,
            _ if !in_escape => count += 1,
            _ => {}
        }
    }
    count
}

fn wrap_text(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in value.split_whitespace() {
        if current.is_empty() {
            push_word_wrapped(&mut lines, &mut current, word, width);
        } else if display_width(&current) + 1 + display_width(word) <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            push_word_wrapped(&mut lines, &mut current, word, width);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn push_word_wrapped(lines: &mut Vec<String>, current: &mut String, word: &str, width: usize) {
    let mut chunk = String::new();
    for character in word.chars() {
        if display_width(&chunk) == width {
            lines.push(std::mem::take(&mut chunk));
        }
        chunk.push(character);
    }
    *current = chunk;
}

fn extract_assignee_name(assignee: &str) -> &str {
    assignee
        .find('<')
        .map(|pos| assignee[..pos].trim())
        .unwrap_or_else(|| assignee.trim())
}

fn status_icon(status: &str) -> &'static str {
    match status {
        "todo" => "○",
        "in-progress" => "→",
        "ready-for-qa" => "◎",
        "done" => "✓",
        "blocked" => "✗",
        _ => "·",
    }
}

fn parse_story_points(story_points: &str) -> usize {
    story_points.trim().parse().unwrap_or(0)
}

fn format_story_points(value: impl std::fmt::Display) -> String {
    format!("◈{value}")
}

fn story_points_column_width<'a>(stories: impl IntoIterator<Item = &'a StoryOverview>) -> usize {
    stories
        .into_iter()
        .map(|story| display_width(&format_story_points(&story.story_points)))
        .max()
        .unwrap_or(0)
}

fn sum_story_points<'a>(stories: impl IntoIterator<Item = &'a StoryOverview>) -> usize {
    stories
        .into_iter()
        .map(|story| parse_story_points(&story.story_points))
        .sum()
}

fn format_story_status_label(story: &StoryOverview, points_width: usize) -> String {
    let points = format_story_points(&story.story_points);
    let padding = " ".repeat(points_width.saturating_sub(display_width(&points)));
    format!("{} {}{}", story.id, padding, points)
}

fn format_colored_story_status_label(
    theme: &Theme,
    story: &StoryOverview,
    points_width: usize,
) -> String {
    let points = format_story_points(&story.story_points);
    let padding = " ".repeat(points_width.saturating_sub(display_width(&points)));
    format!(
        "{} {}{}",
        theme.id(&story.id),
        padding,
        theme.story_points(points)
    )
}

fn sprint_status_label(end_date: &str, readme_status: Option<&str>) -> &'static str {
    if readme_status
        .map(|s| matches!(s, "completed" | "closed" | "done"))
        .unwrap_or(false)
    {
        return "completed";
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
        let today = chrono::Local::now().date_naive();
        if date >= today { "active" } else { "overdue" }
    } else {
        "active"
    }
}

fn render_progress_bar(
    theme: &Theme,
    done: usize,
    in_progress: usize,
    total: usize,
    width: usize,
) -> String {
    let bar_width = (width / 5).clamp(8, 24).saturating_sub(2);
    let body_width = bar_width.saturating_sub(2);
    let total_units = body_width * 8;
    let total = total.max(1);
    let done = done.min(total);
    let active = done.saturating_add(in_progress).min(total);
    let done_units = scaled_bar_units(done, total, total_units);
    let active_units = scaled_bar_units(active, total, total_units);
    let mut bar = String::new();

    let first_segment = progress_segment_for_unit(0, done_units, active_units);
    bar.push_str(&theme.paint(progress_segment_style(first_segment), "\u{e0b6}"));

    for cell in 0..body_width {
        let start = cell * 8;
        let first = progress_segment_for_unit(start, done_units, active_units);
        let split = (1..8)
            .find(|offset| {
                progress_segment_for_unit(start + offset, done_units, active_units) != first
            })
            .unwrap_or(8);
        if split == 8 {
            bar.push_str(&theme.paint(
                progress_segment_style(first),
                progress_segment_full_char(first),
            ));
        } else {
            let next = progress_segment_for_unit(start + split, done_units, active_units);
            bar.push_str(&theme.paint_with_background(
                progress_segment_style(first),
                progress_segment_style(next),
                progress_fraction_char(split),
            ));
        }
    }

    let last_segment =
        progress_segment_for_unit(total_units.saturating_sub(1), done_units, active_units);
    bar.push_str(&theme.paint(progress_segment_style(last_segment), "\u{e0b4}"));

    bar
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ProgressSegment {
    Done,
    InProgress,
    Empty,
}

fn scaled_bar_units(value: usize, total: usize, total_units: usize) -> usize {
    (value * total_units + total / 2) / total
}

fn progress_segment_for_unit(
    unit: usize,
    done_units: usize,
    active_units: usize,
) -> ProgressSegment {
    if unit < done_units {
        ProgressSegment::Done
    } else if unit < active_units {
        ProgressSegment::InProgress
    } else {
        ProgressSegment::Empty
    }
}

fn progress_segment_style(segment: ProgressSegment) -> Style {
    match segment {
        ProgressSegment::Done => Style::Green,
        ProgressSegment::InProgress => Style::Blue,
        ProgressSegment::Empty => Style::DarkGray,
    }
}

fn progress_segment_full_char(segment: ProgressSegment) -> &'static str {
    match segment {
        ProgressSegment::Done | ProgressSegment::InProgress => "█",
        ProgressSegment::Empty => "░",
    }
}

fn progress_fraction_char(units: usize) -> &'static str {
    match units {
        1 => "▏",
        2 => "▎",
        3 => "▍",
        4 => "▌",
        5 => "▋",
        6 => "▊",
        7 => "▉",
        _ => "█",
    }
}

fn format_colored_task_summary(theme: &Theme, summary: Option<&TaskSummary>) -> String {
    summary
        .map(|s| {
            format!(
                "{} {} {} {}",
                theme.paint(Style::Green, format!("✓{}", s.done)),
                theme.paint(Style::Blue, format!("▶{}", s.in_progress)),
                theme.paint(Style::Muted, format!("·{}", s.todo)),
                theme.paint(Style::Red, format!("✗{}", s.blocked)),
            )
        })
        .unwrap_or_else(|| "-".to_string())
}

fn push_sprint_header_band(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    sprint: &SprintOverview,
) {
    let sprint_id = sprint
        .sprint_name
        .split_once('.')
        .map(|(id, _)| id)
        .unwrap_or(&sprint.sprint_name);
    let headline = title_case_headline(&sprint.headline);
    let status_label = sprint_status_label(&sprint.end_date, sprint.readme_status.as_deref());

    // Top separator: ─── S000 · Headline [fill] status ───
    let title_text = format!("{} · {}", sprint_id, headline);
    let prefix_text = format!("─── {title_text} ");
    let suffix_text = format!(" {} ───", status_label);
    let fill = layout
        .width
        .saturating_sub(display_width(&prefix_text) + display_width(&suffix_text));
    let colored_status = match status_label {
        "overdue" => theme.paint(Style::Yellow, status_label),
        "completed" => theme.paint(Style::Muted, status_label),
        _ => status_label.to_string(),
    };
    push_line(
        output,
        &format!(
            "{}{}{} {} {}",
            theme.paint(Style::Muted, "─── "),
            theme.paint(Style::Cyan, title_text),
            theme.paint(Style::Muted, format!(" {}", "─".repeat(fill))),
            colored_status,
            theme.paint(Style::Muted, "───"),
        ),
    );

    push_line(output, "");

    // Counts per status
    let total_points: usize = sprint
        .stories_by_status
        .values()
        .map(|stories| sum_story_points(stories.iter()))
        .sum();
    let done = sprint
        .stories_by_status
        .get("done")
        .map(|v| v.len())
        .unwrap_or(0);
    let done_points = sprint
        .stories_by_status
        .get("done")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);
    let in_progress = sprint
        .stories_by_status
        .get("in-progress")
        .map(|v| v.len())
        .unwrap_or(0);
    let qa = sprint
        .stories_by_status
        .get("ready-for-qa")
        .map(|v| v.len())
        .unwrap_or(0);
    let todo = sprint
        .stories_by_status
        .get("todo")
        .map(|v| v.len())
        .unwrap_or(0);
    let blocked = sprint
        .stories_by_status
        .get("blocked")
        .map(|v| v.len())
        .unwrap_or(0);
    let in_progress_points = sprint
        .stories_by_status
        .get("in-progress")
        .map(|stories| sum_story_points(stories.iter()))
        .unwrap_or(0);

    // Progress line
    let bar = render_progress_bar(
        theme,
        done_points,
        in_progress_points,
        total_points,
        layout.width,
    );
    let pct = done_points
        .checked_mul(100)
        .and_then(|value| value.checked_div(total_points))
        .unwrap_or(0);
    push_line(
        output,
        &format!(
            "  {} → {}   {}  {}  {}",
            sprint.start_date,
            sprint.end_date,
            bar,
            theme.story_points(format!(
                "{} / {}",
                format_story_points(done_points),
                total_points
            )),
            theme.paint(Style::Muted, format!("{pct}%")),
        ),
    );

    // Count line: N done · N in progress · N in qa · N todo · N blocked
    let dot = theme.paint(Style::Muted, "·");
    let segments: Vec<String> = [
        (done, "done", Style::Green),
        (in_progress, "in progress", Style::Blue),
        (qa, "in qa", Style::Purple),
        (todo, "todo", Style::Muted),
        (blocked, "blocked", Style::Red),
    ]
    .into_iter()
    .map(|(count, label, style)| {
        let s = if count == 0 { Style::Muted } else { style };
        theme.paint(s, format!("{count} {label}"))
    })
    .collect();
    push_line(
        output,
        &format!("  {}", segments.join(&format!("  {dot}  "))),
    );

    push_line(output, "");

    // Bottom separator: full-width dashes
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

fn format_compact_task_summary(summary: Option<&TaskSummary>) -> String {
    summary
        .map(|s| format!("✓{} ▶{} ·{} ✗{}", s.done, s.in_progress, s.todo, s.blocked))
        .unwrap_or_else(|| "-".to_string())
}

fn print_phase_overview(theme: &Theme, layout: OutputLayout, phase: &PhaseOverview) {
    print!("{}", render_phase_overview(theme, layout, phase));
}

fn render_phase_overview(theme: &Theme, layout: OutputLayout, phase: &PhaseOverview) -> String {
    let mut output = String::new();
    let grouped = phase_stories_by_epic(phase);
    let story_count = phase.stories.len();
    let drafted_points = phase_story_points_for_statuses(phase, &["draft", "ready"]);
    let planned_points = phase_story_points_for_statuses(phase, &["todo"]);
    let in_progress_points =
        phase_story_points_for_statuses(phase, &["in-progress", "ready-for-qa", "blocked"]);
    let done_points = phase_story_points_for_statuses(phase, &["done"]);
    let total_points = drafted_points + planned_points + in_progress_points + done_points;
    let summary = PhaseHeaderSummary {
        story_count,
        epic_count: grouped.len(),
        drafted_points,
        planned_points,
        in_progress_points,
        done_points,
        total_points,
    };

    push_phase_header_band(&mut output, theme, layout, phase, &summary);

    let points_width = story_points_column_width(phase.stories.iter());
    for (index, (epic_label, stories)) in grouped.iter().enumerate() {
        if index > 0 {
            push_line(&mut output, "");
        }

        let epic_points = sum_story_points(stories.iter().copied());
        push_line(
            &mut output,
            &format!(
                "{}   {}   {}",
                theme.heading(epic_label),
                theme.count(format_story_count(stories.len())),
                theme.story_points(format_story_points(epic_points)),
            ),
        );

        let stories_by_status = phase_stories_by_status(stories);
        for status in phase_status_display_order() {
            let Some(status_stories) = stories_by_status.get(status) else {
                continue;
            };

            push_line(&mut output, "");
            let icon_label = theme.status_text(status, format!("{} {status}", status_icon(status)));
            let status_points = sum_story_points(status_stories.iter().copied());
            push_line(
                &mut output,
                &format!(
                    "{}   {}   {}",
                    icon_label,
                    theme.count(format_story_count(status_stories.len())),
                    theme.story_points(format_story_points(status_points)),
                ),
            );
            push_phase_story_table(
                &mut output,
                theme,
                layout.width,
                status_stories,
                points_width,
            );
        }
    }

    output
}

struct PhaseHeaderSummary {
    story_count: usize,
    epic_count: usize,
    drafted_points: usize,
    planned_points: usize,
    in_progress_points: usize,
    done_points: usize,
    total_points: usize,
}

fn push_phase_header_band(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    phase: &PhaseOverview,
    summary: &PhaseHeaderSummary,
) {
    let prefix_text = format!("─── {} · Phase Overview ", phase.phase);
    let suffix_text = " ───";
    let fill = layout
        .width
        .saturating_sub(display_width(&prefix_text) + display_width(suffix_text));
    push_line(
        output,
        &format!(
            "{}{}",
            theme.paint(Style::Muted, prefix_text),
            theme.paint(Style::Muted, format!("{}{}", "─".repeat(fill), suffix_text)),
        ),
    );
    push_line(
        output,
        &format!(
            "  {}  {}",
            theme.label("Scope:"),
            theme.paint(
                Style::Muted,
                format!("phase backlog grouped by epic ({})", summary.epic_count)
            )
        ),
    );

    let bar = render_progress_bar(
        theme,
        summary.done_points,
        summary.in_progress_points,
        summary.total_points,
        layout.width,
    );
    let pct = summary
        .done_points
        .checked_mul(100)
        .and_then(|value| value.checked_div(summary.total_points))
        .unwrap_or(0);
    let progress_points = format!(
        "{} / {}",
        format_story_points(summary.done_points),
        summary.total_points
    );
    push_line(
        output,
        &format!(
            "  {}  {}  {}",
            theme.label("Progress:"),
            bar,
            format_args!(
                "{}  {}",
                theme.story_points(progress_points),
                theme.paint(Style::Muted, format!("{pct}%"))
            )
        ),
    );

    let dot = theme.paint(Style::Muted, "·");
    let segments = [
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.drafted_points)),
            theme.paint(Style::Yellow, "drafted")
        ),
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.planned_points)),
            theme.paint(Style::Muted, "planned")
        ),
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.in_progress_points)),
            theme.paint(Style::Blue, "in progress")
        ),
        format!(
            "{} {}",
            theme.story_points(format_story_points(summary.done_points)),
            theme.paint(Style::Green, "done")
        ),
    ];
    push_line(
        output,
        &format!("  {}", segments.join(&format!("  {dot}  "))),
    );

    push_line(
        output,
        &format!(
            "  {}  {}  {}",
            theme.count(format_story_count(summary.story_count)),
            theme.paint(Style::Muted, format_epic_count(summary.epic_count)),
            theme.story_points(format!(
                "{} total",
                format_story_points(summary.total_points)
            )),
        ),
    );
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

fn phase_stories_by_epic<'a>(phase: &'a PhaseOverview) -> Vec<(String, Vec<&'a StoryOverview>)> {
    let mut grouped: BTreeMap<String, Vec<&'a StoryOverview>> = BTreeMap::new();
    for story in &phase.stories {
        let label = story_epic_label(story.epic_id.as_deref(), story.epic_title.as_deref())
            .unwrap_or_else(|| "No epic".to_string());
        grouped.entry(label).or_default().push(story);
    }
    grouped.into_iter().collect()
}

fn phase_stories_by_status<'a>(
    stories: &[&'a StoryOverview],
) -> BTreeMap<&'a str, Vec<&'a StoryOverview>> {
    let mut grouped: BTreeMap<&'a str, Vec<&'a StoryOverview>> = BTreeMap::new();
    for story in stories {
        grouped
            .entry(story.status.as_str())
            .or_default()
            .push(*story);
    }
    grouped
}

fn phase_status_display_order() -> &'static [&'static str] {
    &[
        "draft",
        "ready",
        "todo",
        "in-progress",
        "ready-for-qa",
        "blocked",
        "done",
        "dropped",
    ]
}

fn phase_story_points_for_statuses(phase: &PhaseOverview, statuses: &[&str]) -> usize {
    phase
        .stories
        .iter()
        .filter(|story| statuses.contains(&story.status.as_str()))
        .map(|story| parse_story_points(&story.story_points))
        .sum()
}

fn format_epic_count(count: usize) -> String {
    if count == 1 {
        "1 epic".to_string()
    } else {
        format!("{count} epics")
    }
}

fn print_story_list(theme: &Theme, scope: &str, stories: &[StoryOverview]) {
    print!("{}", render_story_list(theme, scope, stories));
}

fn render_story_list(theme: &Theme, scope: &str, stories: &[StoryOverview]) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "{} {}\n",
        theme.label("Stories:"),
        theme.count(stories.len())
    ));
    output.push_str(&format!("{} {scope}\n", theme.label("Scope:")));
    for story in stories {
        let sprint = story.sprint.as_deref().unwrap_or("~");
        output.push_str(&format!(
            "- {} [{}] sprint={} assignee={} {} {}\n",
            theme.id(&story.id),
            theme.status(&story.status),
            sprint,
            story.assignee,
            theme.story_points(format_story_points(&story.story_points)),
            story.title
        ));
    }
    output
}

fn print_story_details(theme: &Theme, layout: OutputLayout, details: &StoryDetails) {
    print!("{}", render_story_details(theme, layout, details));
}

fn render_story_details(theme: &Theme, layout: OutputLayout, details: &StoryDetails) -> String {
    let mut output = String::new();
    push_story_detail_header(&mut output, theme, layout, details);
    push_story_metadata_table(&mut output, theme, layout, details);
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Story Statement",
        details.story_statement.as_deref(),
    );
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Acceptance Criteria",
        details.acceptance_criteria.as_deref(),
    );
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Definition Of Done",
        details.definition_of_done.as_deref(),
    );
    push_story_markdown_section(
        &mut output,
        theme,
        layout,
        "Notes And Open Questions",
        details.notes_and_open_questions.as_deref(),
    );
    push_story_tasks_section(&mut output, theme, layout, details);
    output
}

fn push_story_detail_header(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &StoryDetails,
) {
    let title = format!("{} · {}", details.story.id, details.story.title);
    let status = format!(
        "{} {}",
        status_icon(&details.story.status),
        details.story.status
    );
    let suffix_width = display_width(&status) + 2;
    let title_width = layout.width.saturating_sub(suffix_width).max(1);
    let title_line = wrap_text(&title, title_width)
        .into_iter()
        .next()
        .unwrap_or(title);
    let padding = layout
        .width
        .saturating_sub(display_width(&title_line) + suffix_width);

    push_line(
        output,
        &format!(
            "{}{}  {}",
            highlight_story_id(theme, &title_line),
            " ".repeat(padding),
            theme.status_text(&details.story.status, status)
        ),
    );
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

fn highlight_story_id(theme: &Theme, line: &str) -> String {
    line.split_once(" · ")
        .map(|(id, title)| format!("{} · {}", theme.id(id), theme.heading(title)))
        .unwrap_or_else(|| theme.heading(line))
}

fn push_story_metadata_table(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &StoryDetails,
) {
    push_line(output, "");
    push_line(output, &theme.heading("Overview"));

    let columns = two_column_table_columns(layout.width, 13, "Field", "Value");
    let mut rows = vec![
        metadata_row(
            theme,
            "Status",
            theme.status_text(
                &details.story.status,
                format!(
                    "{} {}",
                    status_icon(&details.story.status),
                    details.story.status
                ),
            ),
            true,
        ),
        metadata_row(
            theme,
            "Sprint",
            details.story.sprint.as_deref().unwrap_or("~").to_string(),
            false,
        ),
        metadata_row(theme, "Assignee", details.story.assignee.clone(), false),
        metadata_row(
            theme,
            "Points",
            theme.story_points(format_story_points(&details.story.story_points)),
            true,
        ),
    ];

    let task_summary = details
        .story
        .task_summary
        .as_ref()
        .map(|summary| format_colored_task_summary(theme, Some(summary)))
        .unwrap_or_else(|| "-".to_string());
    rows.push(metadata_row(theme, "Tasks", task_summary, true));
    if let Some(phase) = story_phase_label(&details.story_file_path) {
        rows.push(metadata_row(theme, "Phase", phase, false));
    }
    if let Some(epic) = story_epic_label(details.epic_id.as_deref(), details.epic_title.as_deref())
    {
        rows.push(metadata_row(theme, "Epic", epic, false));
    }
    rows.push(metadata_row(
        theme,
        "File",
        simplify_story_path(&details.story_file_path),
        false,
    ));
    rows.push(metadata_row(
        theme,
        "Work started",
        details
            .work_started
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("-")
            .to_string(),
        false,
    ));
    rows.push(metadata_row(
        theme,
        "Work done",
        details
            .work_done
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("-")
            .to_string(),
        false,
    ));

    push_wrapped_table(output, theme, &columns, &rows);
}

fn simplify_story_path(path: &Path) -> String {
    path.strip_prefix("delivery/backlog")
        .unwrap_or(path)
        .display()
        .to_string()
}

fn story_phase_label(path: &Path) -> Option<String> {
    let phase_dir = path.iter().nth(2)?.to_string_lossy();
    phase_dir
        .strip_prefix("phase-")
        .and_then(|rest| rest.split_once('-'))
        .map(|(number, slug)| format!("{} {}", number, headline_from_slug(slug)))
}

fn story_epic_label(epic_id: Option<&str>, epic_title: Option<&str>) -> Option<String> {
    let epic_id = epic_id?.trim();
    if epic_id.is_empty() {
        None
    } else {
        let epic_title = epic_title.unwrap_or("").trim();
        if epic_title.is_empty() {
            Some(epic_id.to_string())
        } else {
            Some(format!("{}  {}", epic_id, epic_title))
        }
    }
}

fn headline_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn metadata_row(theme: &Theme, label: &str, value: String, precolored: bool) -> Vec<TableCell> {
    vec![
        TableCell::preformatted(theme.label(label), CellStyle::Precolored),
        if precolored {
            TableCell::preformatted(value, CellStyle::Precolored)
        } else {
            TableCell::new(value)
        },
    ]
}

fn two_column_table_columns(
    width: usize,
    first_width: usize,
    first_title: &str,
    second_title: &str,
) -> Vec<DynamicTableColumn> {
    let available = row_content_width(width, 2);
    let first_width = first_width.min(available.saturating_sub(1)).max(1);
    vec![
        DynamicTableColumn {
            title: first_title.to_string(),
            width: first_width,
        },
        DynamicTableColumn {
            title: second_title.to_string(),
            width: available.saturating_sub(first_width).max(1),
        },
    ]
}

fn push_story_markdown_section(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    title: &str,
    content: Option<&str>,
) {
    let Some(content) = content.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    push_line(output, "");
    push_line(output, &theme.heading(title));
    push_line(output, &theme.paint(Style::Muted, "─".repeat(title.len())));
    push_terminal_markdown(output, theme, layout.width, content);
}

fn push_terminal_markdown(output: &mut String, theme: &Theme, width: usize, content: &str) {
    let mut table_lines = Vec::new();
    let mut code_block = CodeBlockKind::None;

    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if is_markdown_table_line(trimmed) && matches!(code_block, CodeBlockKind::None) {
            table_lines.push(trimmed.to_string());
            continue;
        }
        flush_markdown_table(output, theme, width, &mut table_lines);

        if trimmed.starts_with("```") {
            code_block = toggle_code_block(code_block, trimmed);
            continue;
        }

        if !matches!(code_block, CodeBlockKind::None) {
            push_code_block_line(output, theme, width, line, code_block);
            continue;
        }

        push_terminal_markdown_line(output, theme, width, line);
    }

    flush_markdown_table(output, theme, width, &mut table_lines);
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum CodeBlockKind {
    None,
    Plain,
    Gherkin,
}

fn toggle_code_block(current: CodeBlockKind, fence_line: &str) -> CodeBlockKind {
    if !matches!(current, CodeBlockKind::None) {
        return CodeBlockKind::None;
    }

    let info = fence_line.trim_start_matches('`').trim();
    if info.eq_ignore_ascii_case("gherkin") {
        CodeBlockKind::Gherkin
    } else {
        CodeBlockKind::Plain
    }
}

fn push_code_block_line(
    output: &mut String,
    theme: &Theme,
    width: usize,
    line: &str,
    code_block: CodeBlockKind,
) {
    match code_block {
        CodeBlockKind::Gherkin => push_gherkin_code_line(output, theme, width, line),
        CodeBlockKind::Plain => {
            push_wrapped_hanging_line(output, "  │ ", line, width, |value| theme.path(value));
        }
        CodeBlockKind::None => {}
    }
}

fn push_gherkin_code_line(output: &mut String, theme: &Theme, width: usize, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        push_line(output, "  │");
        return;
    }

    if let Some((keyword, rest)) = gherkin_line(trimmed) {
        push_wrapped_hanging_line(
            output,
            "  │ ",
            &format!("{} {}", theme.label(keyword), clean_inline_markdown(rest)),
            width,
            |value| value.to_string(),
        );
    } else {
        push_wrapped_hanging_line(output, "  │ ", trimmed, width, |value| theme.path(value));
    }
}

fn push_terminal_markdown_line(output: &mut String, theme: &Theme, width: usize, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        push_line(output, "");
        return;
    }

    if let Some(heading) = markdown_heading_text(trimmed) {
        push_wrapped_hanging_line(
            output,
            "",
            &clean_inline_markdown(heading),
            width,
            |value| theme.heading(value),
        );
        return;
    }

    if let Some(quote) = trimmed.strip_prefix('>') {
        push_wrapped_hanging_line(
            output,
            "  │ ",
            &clean_inline_markdown(quote.trim()),
            width,
            |value| theme.path(value),
        );
        return;
    }

    if let Some((marker, value)) = markdown_list_item(trimmed) {
        push_wrapped_hanging_line(
            output,
            &format!("  {marker} "),
            &clean_inline_markdown(value),
            width,
            |value| value.to_string(),
        );
        return;
    }

    if let Some((keyword, rest)) = gherkin_line(trimmed) {
        push_wrapped_hanging_line(
            output,
            &format!("  {} ", theme.label(keyword)),
            &clean_inline_markdown(rest),
            width,
            |value| value.to_string(),
        );
        return;
    }

    push_wrapped_hanging_line(
        output,
        "  ",
        &clean_inline_markdown(trimmed),
        width,
        |value| value.to_string(),
    );
}

fn markdown_heading_text(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ') {
        Some(line[hashes + 1..].trim())
    } else {
        None
    }
}

fn markdown_list_item(line: &str) -> Option<(String, &str)> {
    for (prefix, marker) in [
        ("- [x] ", "☑"),
        ("- [X] ", "☑"),
        ("- [ ] ", "☐"),
        ("* [x] ", "☑"),
        ("* [X] ", "☑"),
        ("* [ ] ", "☐"),
        ("- ", "•"),
        ("* ", "•"),
    ] {
        if let Some(value) = line.strip_prefix(prefix) {
            return Some((marker.to_string(), value.trim()));
        }
    }

    let (number, value) = line.split_once(". ")?;
    if number.chars().all(|ch| ch.is_ascii_digit()) {
        Some((format!("{number}."), value.trim()))
    } else {
        None
    }
}

fn gherkin_line(line: &str) -> Option<(&str, &str)> {
    for keyword in [
        "Feature:",
        "Scenario:",
        "Scenario Outline:",
        "Given",
        "When",
        "Then",
        "And",
        "But",
        "Examples:",
    ] {
        if line == keyword {
            return Some((keyword, ""));
        }
        if let Some(rest) = line.strip_prefix(&format!("{keyword} ")) {
            return Some((keyword, rest.trim()));
        }
    }
    None
}

fn clean_inline_markdown(value: &str) -> String {
    value
        .replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .trim()
        .to_string()
}

fn is_markdown_table_line(line: &str) -> bool {
    line.starts_with('|') && line.matches('|').count() >= 2
}

fn flush_markdown_table(
    output: &mut String,
    theme: &Theme,
    width: usize,
    table_lines: &mut Vec<String>,
) {
    if table_lines.is_empty() {
        return;
    }
    push_markdown_table(output, theme, width, table_lines);
    table_lines.clear();
}

fn push_markdown_table(output: &mut String, theme: &Theme, width: usize, lines: &[String]) {
    let rows = lines
        .iter()
        .filter(|line| !is_markdown_table_separator(line))
        .map(|line| parse_markdown_table_row(line))
        .filter(|cells| !cells.is_empty())
        .collect::<Vec<_>>();
    let Some((header, body)) = rows.split_first() else {
        return;
    };
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    if column_count == 0 {
        return;
    }

    let columns = markdown_table_columns(width, header, body, column_count);
    let body_rows = body
        .iter()
        .map(|row| {
            normalize_markdown_row(row, column_count)
                .into_iter()
                .map(TableCell::new)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    push_wrapped_table(output, theme, &columns, &body_rows);
}

fn parse_markdown_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| clean_inline_markdown(cell.trim()))
        .collect()
}

fn is_markdown_table_separator(line: &str) -> bool {
    line.trim()
        .trim_matches('|')
        .split('|')
        .all(|cell| cell.trim().chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

fn normalize_markdown_row(row: &[String], column_count: usize) -> Vec<String> {
    (0..column_count)
        .map(|index| row.get(index).cloned().unwrap_or_default())
        .collect()
}

fn markdown_table_columns(
    width: usize,
    header: &[String],
    body: &[Vec<String>],
    column_count: usize,
) -> Vec<DynamicTableColumn> {
    let available = row_content_width(width, column_count);
    let min_width = (available / column_count).clamp(1, 8);
    let mut widths = (0..column_count)
        .map(|index| {
            std::iter::once(header.get(index).map(String::as_str).unwrap_or(""))
                .chain(
                    body.iter()
                        .map(move |row| row.get(index).map(String::as_str).unwrap_or("")),
                )
                .map(display_width)
                .max()
                .unwrap_or(min_width)
                .max(min_width)
        })
        .collect::<Vec<_>>();

    while widths.iter().sum::<usize>() > available {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > min_width)
            .max_by_key(|(_, width)| **width)
        else {
            break;
        };
        widths[index] -= 1;
    }

    (0..column_count)
        .map(|index| DynamicTableColumn {
            title: header.get(index).cloned().unwrap_or_default(),
            width: widths.get(index).copied().unwrap_or(min_width),
        })
        .collect()
}

fn push_story_tasks_section(
    output: &mut String,
    theme: &Theme,
    layout: OutputLayout,
    details: &StoryDetails,
) {
    push_line(output, "");
    push_line(output, &theme.heading("Tasks"));
    push_line(output, &theme.paint(Style::Muted, "─────"));
    if details.tasks.is_empty() {
        push_line(output, "  - none");
        return;
    }

    let columns = task_table_columns(layout.width, &details.tasks);
    let rows = details
        .tasks
        .iter()
        .map(|task| {
            vec![
                TableCell::styled(&task.id, CellStyle::Id),
                TableCell::preformatted(
                    theme.status_text(
                        &task.normalized_status,
                        format!(
                            "{} {}",
                            status_icon(&task.normalized_status),
                            task.normalized_status
                        ),
                    ),
                    CellStyle::Precolored,
                ),
                TableCell::new(if task.tags.is_empty() {
                    "-".to_string()
                } else {
                    task.tags.join(", ")
                }),
                TableCell::new(if task.description.trim().is_empty() {
                    task.title.clone()
                } else {
                    format!("{} - {}", task.title, task.description.trim())
                }),
            ]
        })
        .collect::<Vec<_>>();
    push_wrapped_table(output, theme, &columns, &rows);
}

fn task_table_columns(width: usize, tasks: &[kanban_core::Task]) -> Vec<DynamicTableColumn> {
    let available = row_content_width(width, 4);
    let task_width = tasks
        .iter()
        .map(|task| display_width(&task.id))
        .max()
        .unwrap_or(4)
        .clamp(4, 20);
    let status_width = tasks
        .iter()
        .map(|task| {
            display_width(&format!(
                "{} {}",
                status_icon(&task.normalized_status),
                task.normalized_status
            ))
        })
        .max()
        .unwrap_or(6)
        .clamp(6, 16);
    let tags_width = tasks
        .iter()
        .map(|task| display_width(&task.tags.join(", ")))
        .max()
        .unwrap_or(4)
        .clamp(4, 18);
    let description_width = available
        .saturating_sub(task_width + status_width + tags_width)
        .max(20);

    vec![
        DynamicTableColumn {
            title: "Task".to_string(),
            width: task_width,
        },
        DynamicTableColumn {
            title: "Status".to_string(),
            width: status_width,
        },
        DynamicTableColumn {
            title: "Tags".to_string(),
            width: tags_width,
        },
        DynamicTableColumn {
            title: "Description".to_string(),
            width: description_width,
        },
    ]
}

fn print_doctor_findings(theme: &Theme, findings: &[DoctorFinding]) {
    if findings.is_empty() {
        println!("{}", theme.success("No doctor findings."));
        return;
    }

    for finding in findings {
        println!(
            "{} [{}] {}",
            finding.scope,
            theme.severity(&finding.severity),
            highlight_frontmatter_tokens(theme, &finding.message)
        );
    }
}

fn highlight_frontmatter_tokens(theme: &Theme, text: &str) -> String {
    let mut output = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '`' && ch != '"' {
            output.push(ch);
            continue;
        }

        let delimiter = ch;
        let mut token = String::new();
        for next in chars.by_ref() {
            if next == delimiter {
                break;
            }
            token.push(next);
        }
        output.push(delimiter);
        output.push_str(&theme.highlight(token));
        output.push(delimiter);
    }

    output
}

fn format_doctor_rule(theme: &Theme, rule: &str) -> String {
    if let Some((prefix, field_name)) = rule.rsplit_once(':') {
        format!("{prefix}:{}", theme.highlight(field_name))
    } else {
        rule.to_string()
    }
}

fn format_doctor_fix_preview(theme: &Theme, issue: &DoctorIssue) -> String {
    let Some(preview) = &issue.fix_preview else {
        return highlight_frontmatter_tokens(theme, &issue.suggestion);
    };

    let old_value = if preview.old_value.is_empty() {
        "<empty>"
    } else {
        &preview.old_value
    };
    format!(
        "{}: {} -> {}",
        theme.highlight(&preview.field_name),
        theme.highlight(old_value),
        theme.highlight(&preview.new_value)
    )
}

fn print_doctor_issue(theme: &Theme, index: usize, total: usize, issue: &DoctorIssue) {
    println!(
        "{} {} / {}",
        theme.heading("Doctor Issue"),
        theme.count(index),
        theme.count(total)
    );
    println!(
        "{} {}",
        theme.label("Severity:"),
        theme.severity(&issue.severity)
    );
    println!(
        "{} {}",
        theme.label("Rule:"),
        format_doctor_rule(theme, &issue.rule)
    );
    println!("{} {}", theme.label("Scope:"), issue.scope);
    if let Some(story_id) = &issue.story_id {
        println!("{} {}", theme.label("Story:"), theme.id(story_id));
    }
    if let Some(path) = &issue.file_path {
        println!("{} {}", theme.label("File:"), theme.path(path.display()));
    }
    println!(
        "{} {}",
        theme.label("Problem:"),
        highlight_frontmatter_tokens(theme, &issue.message)
    );
    println!(
        "{} {}",
        theme.label("Suggested fix:"),
        format_doctor_fix_preview(theme, issue)
    );
}

fn resolve_doctor_fix_issues(
    repo_root: &PathBuf,
    target: Option<&str>,
) -> Result<Vec<DoctorIssue>> {
    match target.map(str::trim).filter(|value| !value.is_empty()) {
        None => collect_doctor_issues(repo_root),
        Some("current") => collect_doctor_issues_for_current_sprint(repo_root),
        Some(story_id) => collect_doctor_issues_for_story(repo_root, story_id),
    }
}

fn doctor_issue_allows_edit(issue: &DoctorIssue) -> bool {
    !matches!(issue.fix_kind, DoctorFixKind::ManualOnly)
        && (issue.fix_preview.is_some() || !matches!(issue.prompt, DoctorPrompt::None))
}

fn prompt_doctor_fix_action(issue: &DoctorIssue) -> Result<String> {
    loop {
        let input = if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
            prompt("Apply fix? [s]kip / [q]uit: ")?
        } else if doctor_issue_allows_edit(issue) {
            prompt("Apply fix? [y]es / [e]dit / [s]kip / [q]uit: ")?
        } else {
            prompt("Apply fix? [y]es / [s]kip / [q]uit: ")?
        };
        let normalized = if input.trim().is_empty() {
            "y".to_string()
        } else {
            input.trim().to_ascii_lowercase()
        };
        match normalized.as_str() {
            "y" | "yes" if !matches!(issue.fix_kind, DoctorFixKind::ManualOnly) => {
                return Ok(normalized);
            }
            "e" | "edit" if doctor_issue_allows_edit(issue) => return Ok(normalized),
            "s" | "skip" | "q" | "quit" => return Ok(normalized),
            _ => {
                if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
                    println!("Enter skip or quit.");
                } else if doctor_issue_allows_edit(issue) {
                    println!("Enter yes, edit, skip, or quit.");
                } else {
                    println!("Enter yes, skip, or quit.");
                }
            }
        }
    }
}

fn collect_doctor_fix_input(issue: &DoctorIssue) -> Result<DoctorFixInput> {
    let value = match &issue.prompt {
        DoctorPrompt::None => None,
        DoctorPrompt::Text { label, default } => {
            if let Some(default) = default {
                Some(prompt_with_default(label, default)?)
            } else {
                Some(prompt(&format!("{label}: "))?)
            }
        }
        DoctorPrompt::Choice {
            label,
            options,
            default,
        } => loop {
            let options_text = options.join(", ");
            let value = if let Some(default) = default {
                prompt_with_default(label, default)?
            } else {
                prompt(&format!("{label} [{options_text}]: "))?
            };
            if options.iter().any(|option| option == &value) {
                break Some(value);
            }
            println!("Choose one of: {options_text}.");
        },
    };
    Ok(DoctorFixInput { value })
}

fn collect_doctor_edit_input(issue: &DoctorIssue) -> Result<DoctorFixInput> {
    if let Some(preview) = &issue.fix_preview {
        let value =
            prompt_with_default(&format!("{} value", preview.field_name), &preview.new_value)?;
        return Ok(DoctorFixInput { value: Some(value) });
    }

    collect_doctor_fix_input(issue)
}

fn run_doctor_fix_wizard(theme: &Theme, repo_root: &PathBuf, target: Option<&str>) -> Result<()> {
    let mut issues = resolve_doctor_fix_issues(repo_root, target)?;
    if issues.is_empty() {
        println!("{}", theme.success("No doctor findings to fix."));
        return Ok(());
    }

    let mut index = 0;
    while index < issues.len() {
        let total = issues.len();
        let issue = issues[index].clone();
        print_doctor_issue(theme, index + 1, total, &issue);
        if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
            println!(
                "{} Manual-only issue; no automatic fix is available.",
                theme.warning("Note:")
            );
        }
        let action = prompt_doctor_fix_action(&issue)?;
        match action.as_str() {
            "q" | "quit" => {
                println!("{}", theme.warning("Doctor fix aborted."));
                return Ok(());
            }
            "s" | "skip" => {
                index += 1;
            }
            "e" | "edit" => {
                let input = collect_doctor_edit_input(&issue)?;
                let result = apply_doctor_fix(repo_root, &issue, &input)?;
                println!("{} {}", theme.success("Applied:"), result.message);
                for path in result.touched_paths {
                    println!("{} {}", theme.label("Updated:"), theme.path(path.display()));
                }
                issues = resolve_doctor_fix_issues(repo_root, target)?;
                if issues.is_empty() {
                    println!(
                        "{}",
                        theme.success("All scoped doctor findings are resolved.")
                    );
                    return Ok(());
                }
            }
            _ => {
                if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
                    println!("{}", theme.warning("Skipping manual-only issue."));
                    index += 1;
                    continue;
                }
                let input = collect_doctor_fix_input(&issue)?;
                let result = apply_doctor_fix(repo_root, &issue, &input)?;
                println!("{} {}", theme.success("Applied:"), result.message);
                for path in result.touched_paths {
                    println!("{} {}", theme.label("Updated:"), theme.path(path.display()));
                }
                issues = resolve_doctor_fix_issues(repo_root, target)?;
                if issues.is_empty() {
                    println!(
                        "{}",
                        theme.success("All scoped doctor findings are resolved.")
                    );
                    return Ok(());
                }
            }
        }
    }

    println!("{}", theme.success("Doctor fix wizard completed."));
    Ok(())
}

/// ZSH helper functions appended after the clap_complete-generated script.
/// These provide dynamic completion for config keys/values, sprint names, story IDs,
/// doctor fix targets, epic IDs, task statuses, story update option values, and phase IDs.
const ZSH_DYNAMIC_HELPERS: &str = r#"
_kanban_config_keys() {
    local -a keys
    keys=(
        paths.backlog
        paths.sprints
        theme.color_mode
        story_points.allowed_values
        story_points.aliases.XS
        story_points.aliases.S
        story_points.aliases.M
        story_points.aliases.L
        story_points.aliases.XL
    )
    compadd -a keys
}
_kanban_config_values() {
    local key="$words[3]"
    case "$key" in
        theme.color_mode)
            compadd auto always never
            ;;
        paths.backlog|paths.sprints)
            _files -/
            ;;
        *)
            _default
            ;;
    esac
}
_kanban_sprint_names() {
    local -a names
    local name
    while IFS= read -r name; do
        [[ -n "$name" ]] && names+=( "$name" )
    done < <(kanban list-ids sprints 2>/dev/null)
    compadd -a names
}
_kanban_story_ids() {
    local -a ids descriptions
    local id title
    local needle="$PREFIX"
    while IFS=$'\t' read -r id title; do
        [[ -z "$id" ]] && continue
        if [[ -z "$needle" || "$id" == *"$needle"* ]]; then
            ids+=( "$id" )
            if [[ -n "$title" ]]; then
                descriptions+=( "$id -- $title" )
            else
                descriptions+=( "$id" )
            fi
        fi
    done < <(kanban list-ids stories-with-titles 2>/dev/null)
    compadd -d descriptions -a ids
}
_kanban_story_or_epic_ids() {
    local -a ids
    local id needle="$PREFIX"
    while IFS= read -r id; do
        [[ -n "$id" && ( -z "$needle" || "$id" == *"$needle"* ) ]] && ids+=( "$id" )
    done < <(kanban list-ids stories 2>/dev/null)
    while IFS= read -r id; do
        [[ -n "$id" && ( -z "$needle" || "$id" == *"$needle"* ) ]] && ids+=( "$id" )
    done < <(kanban list-ids epics 2>/dev/null)
    compadd -a ids
}
_kanban_story_types() {
    compadd user-story epic
}
_kanban_story_update_statuses() {
    local -a statuses
    statuses=(
        draft
        ready
        todo
        in-progress
        ready-for-qa
        blocked
        done
        dropped
    )
    compadd -a statuses
}
_kanban_story_point_values() {
    local -a values
    local value
    while IFS= read -r value; do
        [[ -n "$value" ]] && values+=( "$value" )
    done < <(kanban config get story_points.allowed_values 2>/dev/null | tr -d '[]",' | tr '[:space:]' '\n')
    compadd -a values
}
_kanban_doctor_fix_targets() {
    local -a ids descriptions
    local id title
    ids=( current )
    descriptions=( "current -- current active sprint" )
    while IFS=$'\t' read -r id title; do
        [[ -z "$id" ]] && continue
        ids+=( "$id" )
        if [[ -n "$title" ]]; then
            descriptions+=( "$id -- $title" )
        else
            descriptions+=( "$id" )
        fi
    done < <(kanban list-ids stories-with-titles 2>/dev/null)
    compadd -d descriptions -a ids
}
_kanban_doctor_command_or_repo_root() {
    _alternative \
        'command:doctor command:(show fix help)' \
        'repo-root:repository root:_files -/'
}
_kanban_epic_ids() {
    local -a ids
    local id
    local needle="$PREFIX"
    while IFS= read -r id; do
        [[ -n "$id" && ( -z "$needle" || "$id" == *"$needle"* ) ]] && ids+=( "$id" )
    done < <(kanban list-ids epics 2>/dev/null)
    compadd -a ids
}
_kanban_task_statuses() {
    local -a statuses
    statuses=(
        todo
        in-progress
        blocked
        done
    )
    compadd -a statuses
}
_kanban_story_statuses() {
    local -a statuses
    statuses=(
        backlog
        todo
        in-progress
        ready-for-qa
        blocked
        done
    )
    compadd -a statuses
}
"#;

/// Enhance the zsh completion script by replacing `_default` completions for
/// sprint name, story ID, story update options, task status, and doctor fix target arguments with dynamic lookup helpers.
fn enhance_zsh_completion(script: &str) -> String {
    let enhanced = script
        // Sprint name arguments
        .replace(
            "'::name -- Sprint name to inspect, for example S001.foundation. Defaults to the current sprint.:_default'",
            "'::name -- Sprint name to inspect, for example S001.foundation. Defaults to the current sprint.:_kanban_sprint_names'",
        )
        .replace(
            "':name -- Sprint name to close and roll over.:_default'",
            "':name -- Sprint name to close and roll over.:_kanban_sprint_names'",
        )
        // Story plan sprint argument
        .replace(
            "':sprint -- Target sprint name or Snnn prefix, for example S001.planning or S001.:_default'",
            "':sprint -- Target sprint name or Snnn prefix, for example S001.planning or S001.:_kanban_sprint_names'",
        )
        // Story update --sprint option
        .replace(
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]:SPRINT:_default'",
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]:SPRINT:_kanban_sprint_names'",
        )
        // Story ID arguments (story show, story move, task add, task update)
        .replace(
            "':id -- Story id to inspect, for example US-F1-053.:_default'",
            "':id -- Story id to inspect, for example US-F1-053.:_kanban_story_ids'",
        )
        .replace(
            "':id -- Story id to update, for example US-F1-053.:_default'",
            "':id -- Story id to update, for example US-F1-053.:_kanban_story_or_epic_ids'",
        )
        .replace(
            "'--id=[Update frontmatter id. Omit VALUE to prompt with the current value.]::ID:_default'",
            "'--id=[Update frontmatter id. Omit VALUE to prompt with the current value.]::ID:_kanban_story_or_epic_ids'",
        )
        .replace(
            "'--type=[Update frontmatter type. Omit VALUE to prompt with the current value.]::TYPE:_default'",
            "'--type=[Update frontmatter type. Omit VALUE to prompt with the current value.]::TYPE:_kanban_story_types'",
        )
        .replace(
            "'--status=[Update frontmatter status. Omit VALUE to prompt with the current value.]::STATUS:_default'",
            "'--status=[Update frontmatter status. Omit VALUE to prompt with the current value.]::STATUS:_kanban_story_update_statuses'",
        )
        .replace(
            "'--epic=[Update frontmatter epic. Omit VALUE to prompt with the current value.]::EPIC:_default'",
            "'--epic=[Update frontmatter epic. Omit VALUE to prompt with the current value.]::EPIC:_kanban_epic_ids'",
        )
        .replace(
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]::SPRINT:_default'",
            "'--sprint=[Update frontmatter sprint. Omit VALUE to prompt with the current value.]::SPRINT:_kanban_sprint_names'",
        )
        .replace(
            "'--story-points=[Update frontmatter story_points. Omit VALUE to prompt with the current value.]::POINTS:_default'",
            "'--story-points=[Update frontmatter story_points. Omit VALUE to prompt with the current value.]::POINTS:_kanban_story_point_values'",
        )
        .replace(
            "':id -- Sprint story id to move, for example US-F1-053.:_default'",
            "':id -- Sprint story id to move, for example US-F1-053.:_kanban_story_ids'",
        )
        // Note: .replace replaces ALL occurrences — intentional for task add + task update
        .replace(
            "':story_id -- Parent story id for the task, for example US-F1-053.:_default'",
            "':story_id -- Parent story id for the task, for example US-F1-053.:_kanban_story_ids'",
        )
        .replace(
            "\":: :_kanban__subcmd__doctor_commands\"",
            "\":: :_kanban_doctor_command_or_repo_root\"",
        )
        .replace(
            "'::target -- Optional scope\\: a story id like US-F1-053 or the literal `current`.:_default'",
            "'::target -- Optional scope\\: a story id like US-F1-053 or the literal `current`.:_kanban_doctor_fix_targets'",
        )
        .replace(
            "':key -- Configuration key, for example paths.backlog or theme.color_mode.:_default'",
            "':key -- Configuration key, for example paths.backlog or theme.color_mode.:_kanban_config_keys'",
        )
        .replace(
            "':value -- Configuration value. Use comma-separated values for story_points.allowed_values.:_default'",
            "':value -- Configuration value. Use comma-separated values for story_points.allowed_values.:_kanban_config_values'",
        )
        // Story move status argument
        .replace(
            "':status -- Target status, for example todo, in-progress, ready-for-qa, done, or blocked.:_default'",
            "':status -- Target status, for example todo, in-progress, ready-for-qa, done, or blocked.:_kanban_story_statuses'",
        )
        // Task add/update status argument and option
         .replace(
             "'--status=[Initial task status to write. Defaults to todo.]:STATUS:_default'",
             "'--status=[Initial task status to write. Defaults to todo.]:STATUS:_kanban_task_statuses'",
         )
         .replace(
             "'--status=[Replacement task status. Omitted means keep the current status.]:STATUS:_default'",
             "'--status=[Replacement task status. Omitted means keep the current status.]:STATUS:_kanban_task_statuses'",
         )
         // Sprint create date options
        .replace(
            "'--number=[Sprint number. Defaults to the next suggested number.]:N:_default'",
            "'--number=[Sprint number. Defaults to the next suggested number.]:N:'",
        )
        .replace(
            "'--headline=[Sprint headline slug. Required in non-interactive mode.]:SLUG:_default'",
            "'--headline=[Sprint headline slug. Required in non-interactive mode.]:SLUG:'",
        )
        .replace(
            "'--start=[Start date. Defaults to the suggested next start date.]:YYYY-MM-DD:_default'",
            "'--start=[Start date. Defaults to the suggested next start date.]:YYYY-MM-DD:'",
        )
        .replace(
            "'--end=[End date. Defaults to the suggested next end date.]:YYYY-MM-DD:_default'",
            "'--end=[End date. Defaults to the suggested next end date.]:YYYY-MM-DD:'",
        )
        // Story update date options
        .replace(
            "'--activated=[Update frontmatter activated. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--activated=[Update frontmatter activated. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--work_started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--work_started=[Update frontmatter work_started. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--work_done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--work_done=[Update frontmatter work_done. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--created=[Update frontmatter created. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--created=[Update frontmatter created. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        .replace(
            "'--updated=[Update frontmatter updated. Omit VALUE to prompt with the current value.]:TIMESTAMP:_default'",
            "'--updated=[Update frontmatter updated. Omit VALUE to prompt with the current value.]:TIMESTAMP:'",
        )
        // Web log lines option
        .replace(
            "'--lines=[Only print the last N log lines.]:N:_default'",
            "'--lines=[Only print the last N log lines.]:N:'",
        );
    format!("{enhanced}{ZSH_DYNAMIC_HELPERS}")
}

/// Inject dynamic completion into a single bash case block identified by its label and opts string.
/// Inserts a story/sprint lookup BEFORE the standard opts fallback at the given word position.
fn inject_bash_dynamic(script: &str, label: &str, opts: &str, kind: &str, pos: usize) -> String {
    let old = format!(
        "        {label})\n            opts=\"{opts}\"\n            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq {pos} ]] ; then\n                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )\n                return 0\n            fi"
    );
    let new = format!(
        "        {label})\n            opts=\"{opts}\"\n            if [[ ${{COMP_CWORD}} -eq {pos} && ${{cur}} != -* ]]; then\n                local -a matches=()\n                local id\n                while IFS= read -r id; do\n                    [[ -n \"$id\" && \"$id\" == *\"${{cur}}\"* ]] && matches+=( \"$id\" )\n                done < <(kanban list-ids {kind} 2>/dev/null)\n                COMPREPLY=( \"${{matches[@]}}\" )\n                return 0\n            fi\n            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq {pos} ]] ; then\n                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )\n                return 0\n            fi"
    );
    if script.contains(&old) {
        script.replacen(&old, &new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_doctor_fix_target(script: &str) -> String {
    let old = r#"        kanban__subcmd__doctor__subcmd__fix)
            opts="-h --format --help [TARGET] [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__doctor__subcmd__fix)
            opts="-h --format --help [TARGET] [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                local -a matches=( current )
                local id
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids stories 2>/dev/null)
                COMPREPLY=( "${matches[@]}" )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_doctor_command_or_repo_root(script: &str) -> String {
    let old = r#"        kanban__subcmd__doctor)
            opts="-h --format --help show fix help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__doctor)
            opts="-h --format --help show fix help"
            doctor_commands="show fix help"
            if [[ ${COMP_CWORD} -eq 2 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "${doctor_commands}" -- "${cur}") $(compgen -d -- "${cur}") )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_config_get(script: &str) -> String {
    let old = r#"        kanban__subcmd__config__subcmd__get)
            opts="-h --format --help <KEY> [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__config__subcmd__get)
            opts="-h --format --help <KEY> [REPO_ROOT]"
            config_keys="paths.backlog paths.sprints theme.color_mode story_points.allowed_values story_points.aliases.XS story_points.aliases.S story_points.aliases.M story_points.aliases.L story_points.aliases.XL"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "${config_keys}" -- "${cur}") )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_config_set(script: &str) -> String {
    let old = r#"        kanban__subcmd__config__subcmd__set)
            opts="-h --format --help <KEY> <VALUE> [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__config__subcmd__set)
            opts="-h --format --help <KEY> <VALUE> [REPO_ROOT]"
            config_keys="paths.backlog paths.sprints theme.color_mode story_points.allowed_values story_points.aliases.XS story_points.aliases.S story_points.aliases.M story_points.aliases.L story_points.aliases.XL"
            color_modes="auto always never"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "${config_keys}" -- "${cur}") )
                return 0
            fi
            if [[ ${COMP_CWORD} -eq 4 && ${cur} != -* ]] ; then
                case "${prev}" in
                    theme.color_mode)
                        COMPREPLY=( $(compgen -W "${color_modes}" -- "${cur}") )
                        return 0
                        ;;
                    paths.backlog|paths.sprints)
                        COMPREPLY=( $(compgen -d -- "${cur}") )
                        return 0
                        ;;
                esac
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_sprint_create(script: &str) -> String {
    let old = r#"        kanban__subcmd__sprint__subcmd__create)
            opts="-h --number --headline --start --end --non-interactive --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --number)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --headline)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --start)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --end)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = format!(
        r#"        kanban__subcmd__sprint__subcmd__create)
            opts="-h --number --headline --start --end --non-interactive --format --help [REPO_ROOT]"
            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${{opts}}" -- "${{cur}}") )
                return 0
            fi
            case "${{prev}}" in
                --number)
                    COMPREPLY=()
                    return 0
                    ;;
                --headline)
                    COMPREPLY=()
                    return 0
                    ;;
                --start)
                    COMPREPLY=( $(compgen -W "{date_placeholder}" -- "${{cur}}") )
                    return 0
                    ;;
                --end)
                    COMPREPLY=( $(compgen -W "{date_placeholder}" -- "${{cur}}") )
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${{opts}}" -- "${{cur}}") )
            return 0"#,
        date_placeholder = BASH_DATE_PLACEHOLDER,
    );
    if script.contains(old) {
        script.replacen(old, &new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_web_log(script: &str) -> String {
    let old = r#"        kanban__subcmd__web__subcmd__log)
            opts="-f -h --lines --follow --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --lines)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__web__subcmd__log)
            opts="-f -h --lines --follow --format --help [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --lines)
                    COMPREPLY=()
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_story_plan(script: &str) -> String {
    let old = r#"        kanban__subcmd__story__subcmd__plan)
             opts="-h --sprint --format --help <ID> [REPO_ROOT]"
             if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                 COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --sprint)
                     COMPREPLY=($(compgen -f "${cur}"))
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    let new = r#"        kanban__subcmd__story__subcmd__plan)
             opts="-h --sprint --format --help <ID> [REPO_ROOT]"
             if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                 COMPREPLY=( $(compgen -W "$(kanban list-ids stories 2>/dev/null)" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --sprint)
                     COMPREPLY=( $(compgen -W "$(kanban list-ids sprints 2>/dev/null)" -- "${cur}") )
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_story_move_status(script: &str) -> String {
    let old = r#"        kanban__subcmd__story__subcmd__move)
             opts="-a -h --assignee --format --help <ID> <STATUS> [REPO_ROOT]"
             if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                 COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --assignee)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    let new = r#"        kanban__subcmd__story__subcmd__move)
             opts="-a -h --assignee --format --help <ID> <STATUS> [REPO_ROOT]"
             story_statuses="backlog todo in-progress ready-for-qa blocked done"
             if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                 COMPREPLY=( $(compgen -W "$(kanban list-ids stories 2>/dev/null)" -- "${cur}") )
                 return 0
             fi
             if [[ ${COMP_CWORD} -eq 4 && ${cur} != -* ]] ; then
                 COMPREPLY=( $(compgen -W "${story_statuses}" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --assignee)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_task_add_status(script: &str) -> String {
    let old = r#"        kanban__subcmd__task__subcmd__add)
             opts="-h --title --status --tags --description --format --help <STORY_ID> [REPO_ROOT]"
             if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                 COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --title)
                     COMPREPLY=()
                     return 0
                     ;;
                 --status)
                     COMPREPLY=()
                     return 0
                     ;;
                 --tags)
                     COMPREPLY=()
                     return 0
                     ;;
                 --description)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    let new = r#"        kanban__subcmd__task__subcmd__add)
             opts="-h --title --status --tags --description --format --help <STORY_ID> [REPO_ROOT]"
             task_statuses="todo in-progress blocked done"
             if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                 COMPREPLY=( $(compgen -W "$(kanban list-ids stories 2>/dev/null)" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --title)
                     COMPREPLY=()
                     return 0
                     ;;
                 --status)
                     COMPREPLY=( $(compgen -W "${task_statuses}" -- "${cur}") )
                     return 0
                     ;;
                 --tags)
                     COMPREPLY=()
                     return 0
                     ;;
                 --description)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_task_update_status(script: &str) -> String {
    let old = r#"        kanban__subcmd__task__subcmd__update)
             opts="-h --title --status --tags --description --format --help <STORY_ID> <TASK_ID> [REPO_ROOT]"
             if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                 COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --title)
                     COMPREPLY=()
                     return 0
                     ;;
                 --status)
                     COMPREPLY=()
                     return 0
                     ;;
                 --tags)
                     COMPREPLY=()
                     return 0
                     ;;
                 --description)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    let new = r#"        kanban__subcmd__task__subcmd__update)
             opts="-h --title --status --tags --description --format --help <STORY_ID> <TASK_ID> [REPO_ROOT]"
             task_statuses="todo in-progress blocked done"
             if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                 COMPREPLY=( $(compgen -W "$(kanban list-ids stories 2>/dev/null)" -- "${cur}") )
                 return 0
             fi
             case "${prev}" in
                 --title)
                     COMPREPLY=()
                     return 0
                     ;;
                 --status)
                     COMPREPLY=( $(compgen -W "${task_statuses}" -- "${cur}") )
                     return 0
                     ;;
                 --tags)
                     COMPREPLY=()
                     return 0
                     ;;
                 --description)
                     COMPREPLY=()
                     return 0
                     ;;
                 --format)
                     COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                     return 0
                     ;;
                 *)
                     COMPREPLY=()
                     ;;
             esac
             COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
             return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

/// Enhance the bash completion script with dynamic sprint name, story ID,
/// and doctor fix target completions.
fn enhance_bash_completion(script: &str) -> String {
    let script = inject_bash_doctor_command_or_repo_root(script);
    let script = inject_bash_sprint_create(&script);
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__sprint__subcmd__show",
        "-h --format --help <NAME> [REPO_ROOT]",
        "sprints",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__sprint__subcmd__rollover",
        "-h --format --help <NAME> [REPO_ROOT]",
        "sprints",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__story__subcmd__show",
        "-h --format --help <ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_story_update_dynamic(&script);
    let script = inject_bash_story_move_status(&script);
    let script = inject_bash_story_plan(&script);
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__task__subcmd__add",
        "-h --title --status --tags --description --format --help <STORY_ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__task__subcmd__update",
        "-h --title --status --tags --description --format --help <STORY_ID> <TASK_ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_task_add_status(&script);
    let script = inject_bash_task_update_status(&script);
    let script = inject_bash_doctor_fix_target(&script);
    let script = inject_bash_config_get(&script);
    let script = inject_bash_config_set(&script);
    inject_bash_web_log(&script)
}

#[allow(dead_code)]
fn inject_bash_story_update(script: &str) -> String {
    let old = r#"        kanban__subcmd__story__subcmd__update)
            opts="-h --id --type --status --epic --sprint --story-points --assignee --activated --work-started --work-done --created --updated --task-file --format --help <ID> [REPO_ROOT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --type)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --status)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --epic)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --sprint)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --story-points)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --assignee)
                    COMPREPLY=()
                    return 0
                    ;;
                --activated)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-started)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-done)
                    COMPREPLY=()
                    return 0
                    ;;
                --created)
                    COMPREPLY=()
                    return 0
                    ;;
                --updated)
                    COMPREPLY=()
                    return 0
                    ;;
                --task-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    let new = r#"        kanban__subcmd__story__subcmd__update)
            opts="-h --id --type --status --epic --sprint --story-points --assignee --activated --work-started --work-done --created --updated --task-file --format --help <ID> [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                local -a matches=()
                local id
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids stories 2>/dev/null)
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids epics 2>/dev/null)
                COMPREPLY=( "${matches[@]}" )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --id)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids stories 2>/dev/null)
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --type)
                    COMPREPLY=( $(compgen -W "user-story epic" -- "${cur}") )
                    return 0
                    ;;
                --status)
                    COMPREPLY=( $(compgen -W "draft ready todo in-progress ready-for-qa blocked done dropped" -- "${cur}") )
                    return 0
                    ;;
                --epic)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --sprint)
                    COMPREPLY=( $(compgen -W "$(kanban list-ids sprints 2>/dev/null)" -- "${cur}") )
                    return 0
                    ;;
                --story-points)
                    COMPREPLY=( $(compgen -W "$(kanban config get story_points.allowed_values 2>/dev/null | tr -d '[]",' | tr '[:space:]' ' ')" -- "${cur}") )
                    return 0
                    ;;
                --assignee)
                    COMPREPLY=()
                    return 0
                    ;;
                --activated)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-started)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-done)
                    COMPREPLY=()
                    return 0
                    ;;
                --created)
                    COMPREPLY=()
                    return 0
                    ;;
                --updated)
                    COMPREPLY=()
                    return 0
                    ;;
                --task-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0"#;
    if script.contains(old) {
        script.replacen(old, new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_story_update_dynamic(script: &str) -> String {
    let start_marker = "        kanban__subcmd__story__subcmd__update)\n";
    let end_marker = "        kanban__subcmd__task)\n";
    let Some(start) = script.find(start_marker) else {
        return script.to_string();
    };
    let Some(end) = script[start..]
        .find(end_marker)
        .map(|offset| start + offset)
    else {
        return script.to_string();
    };

    let replacement = r#"        kanban__subcmd__story__subcmd__update)
            opts="-h --id --type --status --epic --sprint --story-points --assignee --activated --work-started --work-done --created --updated --task-file --format --help <ID> [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                local -a matches=()
                local id
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids stories 2>/dev/null)
                while IFS= read -r id; do
                    [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                done < <(kanban list-ids epics 2>/dev/null)
                COMPREPLY=( "${matches[@]}" )
                return 0
            fi
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --id)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids stories 2>/dev/null)
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --type)
                    COMPREPLY=( $(compgen -W "user-story epic" -- "${cur}") )
                    return 0
                    ;;
                --status)
                    COMPREPLY=( $(compgen -W "draft ready todo in-progress ready-for-qa blocked done dropped" -- "${cur}") )
                    return 0
                    ;;
                --epic)
                    local -a matches=()
                    local id
                    while IFS= read -r id; do
                        [[ -n "$id" && "$id" == *"${cur}"* ]] && matches+=( "$id" )
                    done < <(kanban list-ids epics 2>/dev/null)
                    COMPREPLY=( "${matches[@]}" )
                    return 0
                    ;;
                --sprint)
                    COMPREPLY=( $(compgen -W "$(kanban list-ids sprints 2>/dev/null)" -- "${cur}") )
                    return 0
                    ;;
                --story-points)
                    COMPREPLY=( $(compgen -W "$(kanban config get story_points.allowed_values 2>/dev/null | tr -d '[],\"' | tr '[:space:]' ' ')" -- "${cur}") )
                    return 0
                    ;;
                --assignee)
                    COMPREPLY=()
                    return 0
                    ;;
                --activated)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-started)
                    COMPREPLY=()
                    return 0
                    ;;
                --work-done)
                    COMPREPLY=()
                    return 0
                    ;;
                --created)
                    COMPREPLY=()
                    return 0
                    ;;
                --updated)
                    COMPREPLY=()
                    return 0
                    ;;
                --task-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=( $(compgen -W "human json" -- "${cur}") )
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
"#;

    let mut result =
        String::with_capacity(script.len() + replacement.len().saturating_sub(end - start));
    result.push_str(&script[..start]);
    result.push_str(replacement);
    result.push_str(&script[end..]);
    result
}

fn prompt(message: &str) -> Result<String> {
    use std::io::{self, Write};

    print!("{message}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_with_default(label: &str, default: &str) -> Result<String> {
    let value = prompt(&format!("{label} [{default}]: "))?;
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value)
    }
}

fn story_frontmatter_update_value(
    story: &kanban_core::Story,
    field_name: &str,
    option: &Option<Option<String>>,
) -> Result<Option<(String, String)>> {
    match option {
        None => Ok(None),
        Some(Some(value)) => Ok(Some((field_name.to_string(), value.clone()))),
        Some(None) => {
            let default = story
                .frontmatter
                .get(field_name)
                .cloned()
                .unwrap_or_default();
            let value = prompt_with_default(field_name, &default)?;
            Ok(Some((field_name.to_string(), value)))
        }
    }
}

fn open_story_markdown_in_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("EDITOR").context("$EDITOR must be set to edit story markdown.")?;
    if editor.trim().is_empty() {
        bail!("$EDITOR must not be empty.");
    }

    #[cfg(windows)]
    let status = ProcessCommand::new("cmd")
        .arg("/C")
        .arg(format!("%EDITOR% \"{}\"", path.display()))
        .status()
        .context("run $EDITOR")?;

    #[cfg(not(windows))]
    let status = ProcessCommand::new("sh")
        .arg("-c")
        .arg("exec ${EDITOR} \"$1\"")
        .arg("kanban-editor")
        .arg(path)
        .status()
        .context("run $EDITOR")?;

    if !status.success() {
        bail!("$EDITOR exited with status {status}.");
    }
    Ok(())
}

fn prompt_date(label: &str, default: NaiveDate) -> Result<NaiveDate> {
    loop {
        let input = prompt_with_default(label, &default.format("%Y-%m-%d").to_string())?;
        match NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
            Ok(date) => return Ok(date),
            Err(_) => println!("Enter a date as YYYY-MM-DD."),
        }
    }
}

fn suggested_sprint_defaults(repo_root: &PathBuf) -> Result<(u32, Option<(NaiveDate, NaiveDate)>)> {
    let config = kanban_core::load_kanban_config(repo_root)?;
    if !config.sprints_path().is_dir() {
        return Ok((0, None));
    }
    Ok((
        suggested_next_sprint_number(repo_root)?,
        suggested_next_sprint_dates(repo_root)?,
    ))
}

fn prompt_create_sprint(
    repo_root: &PathBuf,
    suggested_start: Option<NaiveDate>,
    suggested_end: Option<NaiveDate>,
) -> Result<CreateSprintInput> {
    let (suggested_number, repo_suggestion) = suggested_sprint_defaults(repo_root)?;
    let number = loop {
        let value = prompt_with_default("Sprint number", &format!("{suggested_number}"))?;
        match value.parse::<u32>() {
            Ok(number) => break number,
            Err(_) => println!("Enter a numeric sprint number."),
        }
    };
    let today = chrono::Local::now().date_naive();
    let default_start = suggested_start
        .or_else(|| repo_suggestion.map(|(start_date, _)| start_date))
        .unwrap_or(today);
    let start_date = loop {
        let date = prompt_date("Start date", default_start)?;
        if date < today {
            println!("Start date cannot be in the past.");
            continue;
        }
        break date;
    };
    let default_end = suggested_end
        .or_else(|| repo_suggestion.map(|(_, end_date)| end_date))
        .unwrap_or_else(|| suggested_sprint_dates(start_date).1);
    let end_date = loop {
        let date = prompt_date("End date", default_end)?;
        if date <= start_date {
            println!("End date must be after start date.");
            continue;
        }
        break date;
    };
    let headline = prompt("Sprint headline: ")?;
    Ok(CreateSprintInput {
        number,
        start_date,
        end_date,
        headline,
    })
}

fn print_rollover_result(theme: &Theme, result: &RolloverResult) {
    let completed = if result.completed_story_ids.is_empty() {
        "none".to_string()
    } else {
        result.completed_story_ids.join(", ")
    };
    let carried = if result.carried_story_ids.is_empty() {
        "none".to_string()
    } else {
        result.carried_story_ids.join(", ")
    };
    println!(
        "{} {} -> {}",
        theme.success("Rolled sprint"),
        result.from_sprint,
        result.to_sprint
    );
    println!(
        "{} {}",
        theme.label("Created next sprint:"),
        if result.created_next_sprint {
            theme.success("yes")
        } else {
            "no".to_string()
        }
    );
    println!("{} {completed}", theme.label("Completed stories:"));
    println!("{} {carried}", theme.label("Carried stories:"));
}

fn render_styled_output(styled: clap::builder::StyledStr, color: bool) -> String {
    if color {
        styled.ansi().to_string()
    } else {
        styled.to_string()
    }
}

fn render_no_args_help_output(theme: &Theme) -> Result<String> {
    let version = Args::command().render_version().to_string();
    let mut command = Args::command();
    let help = render_styled_output(command.render_help(), theme.color);
    Ok(format!("{version}{help}\n"))
}

fn normalize_args(raw_args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
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
mod tests {
    use super::*;
    use clap::Parser;
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
