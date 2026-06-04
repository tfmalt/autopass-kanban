#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    completion::*, doctor_cli::*, json_out::*, layout::*, prompt::*, render::*, theme::*, web::*,
};
use clap::builder::styling::{AnsiColor, Effects, Style as ClapStyle, Styles};
use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) const CLAP_STYLING: Styles = Styles::styled()
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
pub(crate) struct Args {
    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "human",
        help = "Output format. `json` emits a single machine-readable envelope; human output is the default."
    )]
    pub(crate) format: OutputFormat,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum SprintCommand {
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
pub(crate) enum PhaseCommand {
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
pub(crate) enum StoryCommand {
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
pub(crate) enum TaskCommand {
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

pub(crate) const COMPLETION_HELP: &str = "Generate a shell completion script from the current kanban command tree.\n\nInstall zsh completion — add to ~/.zshrc:\n  eval \"$(kanban completion zsh)\"\n\nInstall bash completion — add to ~/.bashrc or ~/.bash_profile:\n  eval \"$(kanban completion bash)\"\n\nNote on direnv: .envrc is evaluated as bash, so eval \"$(kanban completion zsh)\" cannot\nbe placed there. Add the eval line to ~/.zshrc instead; it runs once per shell.\n\nSupported shells: bash, zsh. The command only prints completion scripts and never edits shell config files.";
pub(crate) const DOCTOR_HELP: &str = "Diagnose and optionally fix repository workflow issues.\n\nUsage shortcuts:\n  kanban doctor [REPO_ROOT]        Same as `kanban doctor show [REPO_ROOT]`\n  kanban doctor help               Print this help text\n\nEffects depend on subcommand; `show` is read-only while `fix` rewrites only the affected markdown files.";
pub(crate) const BASH_DATE_PLACEHOLDER: &str = "YYYY-MM-DD";

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum CompletionTarget {
    Bash,
    Zsh,
    Help,
}

impl CompletionTarget {
    pub(crate) fn generator(self) -> Option<clap_complete::Shell> {
        match self {
            CompletionTarget::Bash => Some(clap_complete::Shell::Bash),
            CompletionTarget::Zsh => Some(clap_complete::Shell::Zsh),
            CompletionTarget::Help => None,
        }
    }
}

/// Kind of IDs to list for shell completion.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum ListIdsKind {
    Sprints,
    Stories,
    StoriesWithTitles,
    Epics,
}

/// Output format for the `--format` global flag.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputFormat {
    Human,
    Json,
}

#[derive(Subcommand)]
pub(crate) enum ConfigCommand {
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
pub(crate) enum DoctorCommand {
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
pub(crate) enum WebCommand {
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
pub(crate) enum Command {
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

pub(crate) fn command_repo_root(command: &Command) -> Option<&PathBuf> {
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

pub(crate) fn theme_for_command(command: &Command) -> Theme {
    let color_mode = command_repo_root(command)
        .and_then(|repo_root| {
            kanban_core::load_kanban_config(repo_root)
                .ok()
                .map(|config| config.theme.color_mode)
        })
        .unwrap_or(ColorMode::Auto);
    Theme::for_stdout(color_mode)
}

pub(crate) fn completion_target_label(target: CompletionTarget) -> &'static str {
    match target {
        CompletionTarget::Bash => "bash",
        CompletionTarget::Zsh => "zsh",
        CompletionTarget::Help => "help",
    }
}

pub(crate) fn list_ids_kind_label(kind: ListIdsKind) -> &'static str {
    match kind {
        ListIdsKind::Sprints => "sprints",
        ListIdsKind::Stories => "stories",
        ListIdsKind::StoriesWithTitles => "stories-with-titles",
        ListIdsKind::Epics => "epics",
    }
}
