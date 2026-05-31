use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Result, bail};
use chrono::NaiveDate;
use clap::builder::styling::{AnsiColor, Effects, Style as ClapStyle, Styles};
use clap::{ArgGroup, CommandFactory, Parser, Subcommand, ValueEnum};
use kanban_core::{
    ColorMode, CreateSprintInput, DoctorFinding, DoctorFixInput, DoctorFixKind, DoctorIssue,
    DoctorPrompt, PhaseOverview, RolloverResult, SprintOverview, StoryDetails, StoryKind,
    StoryOverview, TaskSummary, add_task_to_story,
    apply_doctor_fix, collect_doctor_issues, collect_doctor_issues_for_current_sprint,
    collect_doctor_issues_for_story, create_sprint,
    doctor_repository, find_story, get_config_json, get_config_value, init_config,
    list_all_stories, list_current_sprint_stories, list_epic_ids, list_next_sprint_stories,
    list_sprint_names, list_stories_in_sprint, list_story_completion_items, list_story_ids,
    move_story_to_status_with_assignee, plan_story_into_sprint, rollover_sprint, set_config_value,
    suggested_next_sprint_dates, suggested_next_sprint_number, suggested_sprint_dates,
    summarize_current_sprint, summarize_phase, summarize_sprint, summarize_sprints,
    update_task_in_story, validate_repository,
};

const MIN_TERMINAL_WIDTH: usize = 80;
const DEFAULT_OUTPUT_WIDTH: usize = 100;
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

        let code = match style {
            Style::Bold => "1",
            Style::Muted => "2",
            Style::Blue => "1;34",
            Style::Cyan => "1;36",
            Style::Green => "1;32",
            Style::Purple => "1;35",
            Style::Red => "1;31",
            Style::Yellow => "1;33",
        };
        format!("\x1b[{code}m{value}\x1b[0m")
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

    fn path(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Muted, value)
    }

    fn success(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Green, value)
    }

    fn warning(&self, value: impl std::fmt::Display) -> String {
        self.paint(Style::Yellow, value)
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

#[derive(Copy, Clone)]
struct OutputLayout {
    width: usize,
}

impl OutputLayout {
    fn for_stdout() -> Result<Self> {
        let width = detected_terminal_width().unwrap_or(DEFAULT_OUTPUT_WIDTH);
        if width < MIN_TERMINAL_WIDTH {
            bail!(
                "Terminal width must be at least {MIN_TERMINAL_WIDTH} columns for kanban sprint output; detected {width}."
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
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum SprintCommand {
    #[command(
        about = "Show the current sprint. Effect: read-only inspection of sprint folders and README metadata. Side effects: none."
    )]
    Current {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "List sprint folders. Effect: read-only inspection of the configured sprint path from `.kanban/paths.json`. Side effects: none."
    )]
    List {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Show one sprint summary. Effect: read-only inspection of the selected sprint folder, stories, tasks, and README. Side effects: none."
    )]
    Show {
        #[arg(help = "Sprint folder name to inspect, for example S001.foundation.")]
        name: String,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Create a sprint folder. Effect: writes a sprint README and status folders under the configured sprint path from `.kanban/paths.json`. Side effects: prompts for metadata unless --non-interactive or all of --number/--headline/--start/--end are supplied."
    )]
    Create {
        #[arg(long, value_name = "N", help = "Sprint number. Defaults to the next suggested number.")]
        number: Option<u32>,
        #[arg(long, value_name = "SLUG", help = "Sprint headline slug. Required in non-interactive mode.")]
        headline: Option<String>,
        #[arg(long, value_name = "YYYY-MM-DD", help = "Start date. Defaults to the suggested next start date.")]
        start: Option<String>,
        #[arg(long, value_name = "YYYY-MM-DD", help = "End date. Defaults to the suggested next end date.")]
        end: Option<String>,
        #[arg(long, help = "Do not prompt; build the sprint from flags and suggested defaults.")]
        non_interactive: bool,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Roll unfinished work into the next sprint. Effect: moves unfinished sprint story/task files and updates the closed sprint README. Side effects: may create the next sprint folder."
    )]
    Rollover {
        #[arg(help = "Sprint folder name to close and roll over.")]
        name: String,
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
enum StoryCommand {
    #[command(
        about = "Show one story. Effect: read-only inspection of the preferred sprint copy or backlog story plus acceptance criteria and tasks. Side effects: none."
    )]
    Show {
        #[arg(help = "Story id to inspect, for example US-F1-053.")]
        id: String,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "List stories. Effect: read-only inspection of sprint folders and/or backlog stories. Side effects: none."
    )]
    #[command(group(
        ArgGroup::new("scope")
            .args(["current", "all", "next", "sprint"])
            .multiple(false)
    ))]
    List {
        #[arg(long, help = "List stories in the current or active sprint.")]
        current: bool,
        #[arg(long, help = "List all stories across backlog and sprint copies.")]
        all: bool,
        #[arg(
            long,
            help = "List stories in the next sprint after the current sprint."
        )]
        next: bool,
        #[arg(
            long,
            value_name = "ID",
            help = "List stories in the specified sprint folder, for example S001.foundation."
        )]
        sprint: Option<String>,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Move a sprint story to another status. Effect: moves the story and task file between sprint status folders and updates frontmatter. Side effects: in-progress sets assignee/work_started; done refreshes work_done."
    )]
    Move {
        #[arg(help = "Sprint story id to move, for example US-F1-053.")]
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
        about = "Plan a backlog story into a sprint. Effect: moves the backlog story (and its .tasks.md, if present) into the sprint's 01.todo folder and updates frontmatter (status=todo, sprint, activated, updated). Side effects: none beyond the file move."
    )]
    Plan {
        #[arg(help = "Backlog story id to plan, for example US-F2-001.")]
        id: String,
        #[arg(
            long,
            value_name = "SPRINT",
            help = "Target sprint folder name or Snnn prefix, for example S001.planning or S001."
        )]
        sprint: String,
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
        about = "Inspect and maintain sprint folders. Effects depend on subcommand; write subcommands state their markdown side effects."
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
        about = "Inspect or move user stories. Effects depend on subcommand; move mutates sprint/backlog markdown frontmatter and file placement."
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
            | SprintCommand::Rollover { repo_root, .. } => Some(repo_root),
        },
        Command::Phase { command } => match command {
            PhaseCommand::Show { repo_root, .. } => Some(repo_root),
        },
        Command::Story { command } => match command {
            StoryCommand::Show { repo_root, .. }
            | StoryCommand::List { repo_root, .. }
            | StoryCommand::Move { repo_root, .. }
            | StoryCommand::Plan { repo_root, .. } => Some(repo_root),
        },
        Command::Task { command } => match command {
            TaskCommand::Add { repo_root, .. } | TaskCommand::Update { repo_root, .. } => {
                Some(repo_root)
            }
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

fn print_sprint_overview(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) {
    print!("{}", render_sprint_overview(theme, layout, sprint));
}

fn render_sprint_overview(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) -> String {
    let mut output = String::new();

    // Dashboard header band: top separator, progress line, count line, bottom separator
    push_sprint_header_band(&mut output, theme, layout, sprint);

    // Sprint goal (below bottom separator)
    if let Some(goal) = &sprint.sprint_goal {
        push_wrapped_label_value(&mut output, theme, "Sprint Goal:", goal, layout.width);
    }

    // Warnings
    if !sprint.warnings.is_empty() {
        push_line(&mut output, "");
        for warning in &sprint.warnings {
            push_wrapped_hanging_line(&mut output, "", warning, layout.width, |v| theme.warning(v));
        }
    }

    // Status sections: todo, in-progress, ready-for-qa (expanded with story rows)
    for status in ["todo", "in-progress", "ready-for-qa"] {
        let stories = sprint
            .stories_by_status
            .get(status)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        push_line(&mut output, "");
        let icon_label = theme.status_text(status, format!("{} {status}", status_icon(status)));
        if stories.is_empty() {
            push_line(
                &mut output,
                &format!("{icon_label}  {}  ·  none", theme.count(0)),
            );
        } else {
            push_line(
                &mut output,
                &format!("{icon_label}  {}", theme.count(stories.len())),
            );
            push_story_table(&mut output, theme, layout.width, stories);
        }
    }

    // Summary footer: ✓ done N   ✗ blocked N
    let done_count = sprint
        .stories_by_status
        .get("done")
        .map(|v| v.len())
        .unwrap_or(0);
    let blocked_count = sprint
        .stories_by_status
        .get("blocked")
        .map(|v| v.len())
        .unwrap_or(0);
    push_line(&mut output, "");
    let done_part = theme.paint(Style::Green, format!("{} done", status_icon("done")));
    let blocked_style = if blocked_count > 0 {
        Style::Red
    } else {
        Style::Muted
    };
    let blocked_part = theme.paint(blocked_style, format!("{} blocked", status_icon("blocked")));
    push_line(
        &mut output,
        &format!(
            "  {}  {}   {}  {}",
            done_part,
            theme.count(done_count),
            blocked_part,
            theme.count(blocked_count),
        ),
    );

    // Blocked work detail callout
    push_line(&mut output, "");
    push_line(&mut output, &theme.heading("Blocked work"));
    if sprint.blocked_work.is_empty() {
        push_line(&mut output, "  - none");
    } else {
        push_blocked_work_table(&mut output, theme, layout.width, &sprint.blocked_work);
    }

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

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_wrapped_label_value(
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
            push_line(output, &format!("{} {line}", theme.label(label)));
        } else {
            push_line(output, &format!("{}{line}", " ".repeat(prefix_width)));
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
}

impl TableCell {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
        }
    }

    fn styled(text: impl Into<String>, style: CellStyle) -> Self {
        Self {
            text: text.into(),
            style: Some(style),
        }
    }
}

fn push_story_table(output: &mut String, theme: &Theme, width: usize, stories: &[StoryOverview]) {
    let columns = story_table_columns(width, stories);
    let rows = stories
        .iter()
        .map(|story| {
            vec![
                TableCell::styled(format_story_status_label(story), CellStyle::Id),
                TableCell::new(&story.title),
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

fn story_table_columns(width: usize, stories: &[StoryOverview]) -> Vec<(&'static str, usize)> {
    let available = row_content_width(width, 4);
    let id_width = stories
        .iter()
        .map(|story| display_width(&format_story_status_label(story)))
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

fn push_wrapped_table_row(
    output: &mut String,
    theme: &Theme,
    columns: &[(&'static str, usize)],
    row: &[TableCell],
) {
    let wrapped_cells = row
        .iter()
        .zip(columns)
        .map(|(cell, (_, width))| wrap_text(&cell.text, *width))
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

fn sum_story_points<'a>(stories: impl IntoIterator<Item = &'a StoryOverview>) -> usize {
    stories
        .into_iter()
        .map(|story| parse_story_points(&story.story_points))
        .sum()
}

fn format_story_status_label(story: &StoryOverview) -> String {
    format!("{} ({}pt)", story.id, story.story_points)
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

fn render_progress_bar(theme: &Theme, done: usize, total: usize, width: usize) -> String {
    let bar_width = (width / 8).clamp(8, 20);
    let filled = if total == 0 {
        0
    } else {
        done * bar_width / total
    };
    let empty = bar_width.saturating_sub(filled);
    format!(
        "{}{}",
        theme.paint(Style::Blue, "█".repeat(filled)),
        theme.paint(Style::Muted, "░".repeat(empty)),
    )
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
    let prefix_text = format!("─── {} · {} ", sprint_id, headline);
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
            "{} {} {}",
            theme.paint(Style::Muted, format!("{}{}", prefix_text, "─".repeat(fill))),
            colored_status,
            theme.paint(Style::Muted, "───"),
        ),
    );

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

    // Progress line
    let bar = render_progress_bar(theme, done_points, total_points, layout.width);
    let pct = if total_points == 0 {
        0
    } else {
        done_points * 100 / total_points
    };
    push_line(
        output,
        &format!(
            "  {} → {}   {}  {} / {}  {}",
            sprint.start_date,
            sprint.end_date,
            bar,
            theme.count(done_points),
            theme.count(total_points),
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

    // Bottom separator: full-width dashes
    push_line(output, &theme.paint(Style::Muted, "─".repeat(layout.width)));
}

fn format_compact_task_summary(summary: Option<&TaskSummary>) -> String {
    summary
        .map(|s| format!("✓{} ▶{} ·{} ✗{}", s.done, s.in_progress, s.todo, s.blocked))
        .unwrap_or_else(|| "-".to_string())
}

fn print_phase_overview(theme: &Theme, phase: &PhaseOverview) {
    println!("{} {}", theme.label("Phase:"), theme.id(&phase.phase));
    println!(
        "{} {}",
        theme.label("Stories:"),
        theme.count(phase.stories.len())
    );
    for story in &phase.stories {
        let sprint = story.sprint.as_deref().unwrap_or("~");
        println!(
            "- {} [{}] sprint={} assignee={} points={} {}",
            theme.id(&story.id),
            theme.status(&story.status),
            sprint,
            story.assignee,
            theme.count(&story.story_points),
            story.title
        );
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
            "- {} [{}] sprint={} assignee={} points={} {}\n",
            theme.id(&story.id),
            theme.status(&story.status),
            sprint,
            story.assignee,
            theme.count(&story.story_points),
            story.title
        ));
    }
    output
}

fn print_story_details(theme: &Theme, details: &StoryDetails) {
    let kind = match details.story.kind {
        StoryKind::Backlog => "backlog",
        StoryKind::Sprint => "sprint",
    };

    println!("{} {}", theme.label("Story:"), theme.id(&details.story.id));
    println!("{} {}", theme.label("Title:"), details.story.title);
    println!("{} {kind}", theme.label("Kind:"));
    println!(
        "{} {}",
        theme.label("Status:"),
        theme.status(&details.story.status)
    );
    println!("{} {}", theme.label("Assignee:"), details.story.assignee);
    println!(
        "{} {}",
        theme.label("Story points:"),
        theme.count(&details.story.story_points)
    );
    println!(
        "{} {}",
        theme.label("Path:"),
        theme.path(details.story.relative_path.display())
    );

    if let Some(sprint) = &details.story.sprint {
        println!("{} {sprint}", theme.label("Sprint:"));
    }
    if let Some(source_path) = &details.source_story_path {
        println!(
            "{} {}",
            theme.label("Source story:"),
            theme.path(source_path.display())
        );
    }
    if let Some(task_file_path) = &details.task_file_path {
        println!(
            "{} {}",
            theme.label("Task file:"),
            theme.path(task_file_path.display())
        );
    }
    if let Some(summary) = &details.story.task_summary {
        println!(
            "{} {}",
            theme.label("Task summary:"),
            theme.path(format_task_summary(summary))
        );
    }

    print_optional_section(theme, "Story Statement", details.story_statement.as_deref());
    print_optional_section(
        theme,
        "Acceptance Criteria",
        details.acceptance_criteria.as_deref(),
    );
    print_optional_section(
        theme,
        "Definition Of Done",
        details.definition_of_done.as_deref(),
    );
    print_optional_section(
        theme,
        "Notes And Open Questions",
        details.notes_and_open_questions.as_deref(),
    );

    println!("{}", theme.heading("Tasks"));
    if details.tasks.is_empty() {
        println!("- none");
    } else {
        for task in &details.tasks {
            println!(
                "- {} [{}] {}",
                theme.id(&task.id),
                theme.status(&task.normalized_status),
                task.title
            );
        }
    }
}

fn print_optional_section(theme: &Theme, title: &str, content: Option<&str>) {
    if let Some(content) = content {
        println!("{}", theme.heading(format!("{title}:")));
        println!("{content}");
    }
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
            finding.message
        );
    }
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
    println!("{} {}", theme.label("Rule:"), issue.rule);
    println!("{} {}", theme.label("Scope:"), issue.scope);
    if let Some(story_id) = &issue.story_id {
        println!("{} {}", theme.label("Story:"), theme.id(story_id));
    }
    if let Some(path) = &issue.file_path {
        println!("{} {}", theme.label("File:"), theme.path(path.display()));
    }
    println!("{} {}", theme.label("Problem:"), issue.message);
    println!("{} {}", theme.label("Suggested fix:"), issue.suggestion);
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

fn prompt_doctor_fix_action(issue: &DoctorIssue) -> Result<String> {
    loop {
        let input = prompt("Apply fix? [y]es / [s]kip / [q]uit: ")?;
        let normalized = if input.trim().is_empty() {
            "y".to_string()
        } else {
            input.trim().to_ascii_lowercase()
        };
        match normalized.as_str() {
            "y" | "yes" | "s" | "skip" | "q" | "quit" => return Ok(normalized),
            _ => {
                if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
                    println!("Enter skip or quit.");
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

fn format_task_summary(summary: &TaskSummary) -> String {
    format!(
        "tasks(todo={}, in-progress={}, blocked={}, done={})",
        summary.todo, summary.in_progress, summary.blocked, summary.done
    )
}

/// ZSH helper functions appended after the clap_complete-generated script.
/// These provide dynamic completion for config keys/values, sprint names, story IDs,
/// doctor fix targets, and epic IDs.
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
    while IFS= read -r id; do
        [[ -n "$id" ]] && ids+=( "$id" )
    done < <(kanban list-ids epics 2>/dev/null)
    compadd -a ids
}
"#;

/// Enhance the zsh completion script by replacing `_default` completions for
/// sprint name, story ID, and doctor fix target arguments with dynamic lookup helpers.
fn enhance_zsh_completion(script: &str) -> String {
    let enhanced = script
        // Sprint name arguments
        .replace(
            "':name -- Sprint folder name to inspect, for example S001.foundation.:_default'",
            "':name -- Sprint folder name to inspect, for example S001.foundation.:_kanban_sprint_names'",
        )
        .replace(
            "':name -- Sprint folder name to close and roll over.:_default'",
            "':name -- Sprint folder name to close and roll over.:_kanban_sprint_names'",
        )
        // Story ID arguments (story show, story move, task add, task update)
        .replace(
            "':id -- Story id to inspect, for example US-F1-053.:_default'",
            "':id -- Story id to inspect, for example US-F1-053.:_kanban_story_ids'",
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
        "        {label})\n            opts=\"{opts}\"\n            if [[ ${{COMP_CWORD}} -eq {pos} && ${{cur}} != -* ]]; then\n                COMPREPLY=( $(compgen -W \"$(kanban list-ids {kind} 2>/dev/null)\" -- \"${{cur}}\") )\n                return 0\n            fi\n            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq {pos} ]] ; then\n                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )\n                return 0\n            fi"
    );
    if script.contains(&old) {
        script.replacen(&old, &new, 1)
    } else {
        script.to_string()
    }
}

fn inject_bash_doctor_fix_target(script: &str) -> String {
    let old = r#"        kanban__subcmd__doctor__subcmd__fix)
            opts="-h --help [TARGET] [REPO_ROOT]"
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
    let new = r#"        kanban__subcmd__doctor__subcmd__fix)
            opts="-h --help [TARGET] [REPO_ROOT]"
            if [[ ${COMP_CWORD} -eq 3 && ${cur} != -* ]] ; then
                COMPREPLY=( $(compgen -W "current $(kanban list-ids stories 2>/dev/null)" -- "${cur}") )
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

fn inject_bash_doctor_command_or_repo_root(script: &str) -> String {
    let old = r#"        kanban__subcmd__doctor)
            opts="-h --help show fix help"
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
    let new = r#"        kanban__subcmd__doctor)
            opts="-h --help show fix help"
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
            opts="-h --help <KEY> [REPO_ROOT]"
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
    let new = r#"        kanban__subcmd__config__subcmd__get)
            opts="-h --help <KEY> [REPO_ROOT]"
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
            opts="-h --help <KEY> <VALUE> [REPO_ROOT]"
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
    let new = r#"        kanban__subcmd__config__subcmd__set)
            opts="-h --help <KEY> <VALUE> [REPO_ROOT]"
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

/// Enhance the bash completion script with dynamic sprint name, story ID,
/// and doctor fix target completions.
fn enhance_bash_completion(script: &str) -> String {
    let script = inject_bash_doctor_command_or_repo_root(script);
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__sprint__subcmd__show",
        "-h --help <NAME> [REPO_ROOT]",
        "sprints",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__sprint__subcmd__rollover",
        "-h --help <NAME> [REPO_ROOT]",
        "sprints",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__story__subcmd__show",
        "-h --help <ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__story__subcmd__move",
        "-a -h --assignee --help <ID> <STATUS> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__task__subcmd__add",
        "-h --title --status --tags --description --help <STORY_ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_dynamic(
        &script,
        "kanban__subcmd__task__subcmd__update",
        "-h --title --status --tags --description --help <STORY_ID> <TASK_ID> [REPO_ROOT]",
        "stories",
        3,
    );
    let script = inject_bash_doctor_fix_target(&script);
    let script = inject_bash_config_get(&script);
    inject_bash_config_set(&script)
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
            SprintCommand::Show { name, repo_root } => {
                let sprint = summarize_sprint(repo_root, &name)?;
                print_sprint_overview(&theme, OutputLayout::for_stdout()?, &sprint);
            }
            SprintCommand::Create {
                number,
                headline,
                start,
                end,
                non_interactive,
                repo_root,
            } => {
                let any_flag = number.is_some() || headline.is_some() || start.is_some() || end.is_some();
                let input = if non_interactive || any_flag {
                    let headline = headline.ok_or_else(|| {
                        anyhow::anyhow!("--headline is required when creating a sprint non-interactively.")
                    })?;
                    let number = match number {
                        Some(value) => value,
                        None => suggested_sprint_defaults(&repo_root)?.0,
                    };
                    let repo_suggestion = suggested_sprint_defaults(&repo_root)?.1;
                    let today = chrono::Local::now().date_naive();
                    let start_date = match start {
                        Some(value) => NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
                            .map_err(|_| anyhow::anyhow!("--start must be a date as YYYY-MM-DD."))?,
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
        },
        Command::Phase { command } => match command {
            PhaseCommand::Show { phase, repo_root } => {
                let phase = summarize_phase(repo_root, &phase)?;
                print_phase_overview(&theme, &phase);
            }
        },
        Command::Story { command } => match command {
            StoryCommand::Show { id, repo_root } => match find_story(repo_root, &id)? {
                Some(details) => print_story_details(&theme, &details),
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
                    println!("{} {}", theme.label("Tasks:"), theme.path(task_path.display()));
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
    fn sprint_overview_wraps_story_rows_to_terminal_width() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F1-999".to_string(),
                title: "Improve current sprint terminal rendering so story descriptions wrap responsively inside the detected table boundary".to_string(),
                status: "in-progress".to_string(),
                assignee: "Ada Lovelace <ada@example.test>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S999.test".to_string()),
                kind: StoryKind::Sprint,
                relative_path: PathBuf::from("doc/backlog/sprints/S999.test/02.in-progress/US-F1-999.md"),
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
            readme_path: PathBuf::from("doc/backlog/sprints/S999.test/README.md"),
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
            readme_path: PathBuf::from("doc/backlog/sprints/S001.foundation/README.md"),
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
        let bar_80 = render_progress_bar(&theme, 6, 14, 80);
        let bar_120 = render_progress_bar(&theme, 6, 14, 120);
        assert_eq!(display_width(&bar_80), 80 / 8);
        assert_eq!(display_width(&bar_120), 120 / 8);
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
                assignee: "TBD".to_string(),
                story_points: "8".to_string(),
                sprint: Some("S001.test".to_string()),
                kind: StoryKind::Sprint,
                relative_path: PathBuf::from("04.done/US-F1-001.md"),
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
                assignee: "TBD".to_string(),
                story_points: "2".to_string(),
                sprint: Some("S001.test".to_string()),
                kind: StoryKind::Sprint,
                relative_path: PathBuf::from("01.todo/US-F1-002.md"),
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
            output.contains("8 / 10"),
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
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F1-002".to_string(),
                title: "A story in progress".to_string(),
                status: "in-progress".to_string(),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S001.test".to_string()),
                kind: StoryKind::Sprint,
                relative_path: PathBuf::from("02.in-progress/US-F1-002.md"),
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
            output.contains("US-F1-002 (3pt)"),
            "story row should include story points: {output}"
        );
    }

    #[test]
    fn done_section_collapsed_in_overview() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "done".to_string(),
            vec![StoryOverview {
                id: "US-F1-001".to_string(),
                title: "A completed story".to_string(),
                status: "done".to_string(),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "2".to_string(),
                sprint: Some("S001.test".to_string()),
                kind: StoryKind::Sprint,
                relative_path: PathBuf::from("04.done/US-F1-001.md"),
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
        // Done count appears in summary line and header band
        assert!(output.contains("✓ done"), "done summary line missing");
        // But the story itself should NOT appear as an individual row
        assert!(
            !output.contains("A completed story"),
            "done story listed individually"
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
            output.lines().any(|line| line == "○ todo  0  ·  none"),
            "todo section should be flush-left"
        );
        assert!(
            output.contains("none"),
            "none placeholder missing for empty section"
        );
    }

    #[test]
    fn warnings_and_status_headers_are_flush_left() {
        let theme = Theme::plain();
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert(
            "in-progress".to_string(),
            vec![StoryOverview {
                id: "US-F1-002".to_string(),
                title: "A story in progress".to_string(),
                status: "in-progress".to_string(),
                assignee: "Someone <s@example.com>".to_string(),
                story_points: "3".to_string(),
                sprint: Some("S001.test".to_string()),
                kind: StoryKind::Sprint,
                relative_path: PathBuf::from("02.in-progress/US-F1-002.md"),
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

        assert!(
            output.lines().any(|line| line == "A warning line"),
            "warning should be flush-left"
        );
        assert!(
            output.lines().any(|line| line == "→ in-progress  1"),
            "status header should be flush-left"
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
    fn no_args_output_starts_with_version_line() {
        let output =
            render_no_args_help_output(&Theme::plain()).expect("no-args output should render");
        let first_line = output.lines().next().expect("output should have lines");

        assert_eq!(
            first_line,
            Args::command().render_version().to_string().trim_end()
        );
        assert!(output.contains("Usage: kanban <COMMAND>"));
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
            assignee: "Ada Lovelace <ada@example.test>".to_string(),
            story_points: "3".to_string(),
            sprint: Some("S000.getting-started".to_string()),
            kind: StoryKind::Sprint,
            relative_path: PathBuf::from(
                "doc/backlog/sprints/S000.getting-started/02.in-progress/US-F1-010.md",
            ),
            task_summary: None,
            task_count: 0,
        }];

        let output = render_story_list(&theme, "active sprint (S000.getting-started)", &stories);

        assert!(output.contains("Stories: 1"));
        assert!(output.contains("Scope: active sprint (S000.getting-started)"));
        assert!(output.contains("US-F1-010 [in-progress] sprint=S000.getting-started"));
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
}
