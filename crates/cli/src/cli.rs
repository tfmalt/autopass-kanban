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

pub(crate) const ROOT_HELP_GIT_REQUIREMENT: &str = "Git requirement:\n  Most `kanban` commands must be run inside a git repository.\n  Run `git init` before `kanban init` or other repository commands.";

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
#[command(after_help = ROOT_HELP_GIT_REQUIREMENT)]
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
        about = "List sprint files. Effect: read-only inspection of the configured sprint path from `.kanban/settings.json`. Side effects: none."
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
        about = "Create a sprint file. Effect: writes one S###.slug.md file under the configured sprint path from `.kanban/settings.json`. Side effects: prompts for metadata unless --non-interactive or at least one of --number/--headline/--start/--end is supplied.",
        long_about = "Create a sprint file. Effect: writes one S###.slug.md file under the configured sprint path from `.kanban/settings.json`. Side effects: prompts for metadata unless --non-interactive or at least one of --number/--headline/--start/--end is supplied.\n\nNon-interactive behavior:\n  `--headline` is required whenever flags are used to build the sprint without prompts.\n  `--number` defaults to the next suggested sprint number.\n  `--start` defaults to the suggested next start date, or today if no sprint history exists.\n  `--end` defaults to the suggested next end date, or a derived end date from the chosen start date.\n\nExample:\n  kanban sprint create --non-interactive --headline foundation --start 2026-06-01 --end 2026-06-12"
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
        about = "Regenerate linked user-story tables in all sprint files. Effect: rewrites only generated ## User Stories selected for sprint blocks."
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
pub(crate) enum EpicCommand {
    #[command(
        about = "Show one epic. Effect: read-only inspection of the canonical epic file plus child story progress and key sections. Side effects: none."
    )]
    Show {
        #[arg(help = "Epic id to inspect, for example EP-F1-06.")]
        id: String,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        visible_aliases = ["edit"],
        about = "Update an epic. With no field options, opens $EDITOR for the epic markdown. Field options update frontmatter; omit an option value to be prompted with the current value as default."
    )]
    Update {
        #[arg(help = "Epic id to update, for example EP-F1-02.")]
        id: String,
        #[arg(long, num_args = 0..=1, value_name = "RANK", help = "Update frontmatter priority (non-negative integer). Omit VALUE to prompt.")]
        priority: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "DATE", help = "Update frontmatter planned_start. Omit VALUE to prompt with the current value.")]
        planned_start: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "DATE", help = "Update frontmatter planned_end. Omit VALUE to prompt with the current value.")]
        planned_end: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "TIMESTAMP", help = "Update frontmatter work_started. Omit VALUE to prompt with the current value.")]
        work_started: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "TIMESTAMP", help = "Update frontmatter work_done. Omit VALUE to prompt with the current value.")]
        work_done: Option<Option<String>>,
        #[arg(default_value = ".", help = "Repository root to update.")]
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
        about = "Move a story to another status. Effect: updates the canonical story frontmatter and regenerates the sprint story table. Side effects: in-progress preserves an existing assignee or sets one if missing, and sets work_started; done refreshes work_done."
    )]
    Move {
        #[arg(help = "Story id to move, for example US-F1-053.")]
        id: String,
        #[arg(
            help = "Target status, for example backlog, ready, planned, todo, in-progress, ready-for-qa, done, or blocked."
        )]
        status: String,
        #[arg(
            short,
            long,
            value_name = "NAME <EMAIL>",
            help = "Override assignee when moving to in-progress. Use `Name <email>` or a comma-separated list of assignees; invalid values fail before files are moved."
        )]
        assignee: Option<String>,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Plan a backlog story into a sprint. Effect: updates the canonical story frontmatter (status=todo, sprint, activated, updated) and regenerates the sprint story table. Side effects: none beyond those markdown updates."
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
        visible_aliases = ["remove", "rm"],
        about = "Delete a story. Effect: removes the canonical story markdown and its sibling .tasks.md file if present, then regenerates the sprint story table when the story belongs to a sprint."
    )]
    Delete {
        #[arg(help = "Story id to delete, for example US-F1-053.")]
        id: String,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        visible_aliases = ["edit"],
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
        #[arg(long, num_args = 0..=1, value_name = "RANK", help = "Update frontmatter priority (non-negative integer). Omit VALUE to prompt with the current value.")]
        priority: Option<Option<String>>,
        #[arg(long, num_args = 0..=1, value_name = "ASSIGNEE", help = "Update frontmatter assignee. Use `Name <email>` or a comma-separated list. Omit VALUE to prompt with the current value.")]
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
        about = "Show story tasks. Effect: reads the story's parsed task log and prints it in human or JSON format. Side effects: none."
    )]
    Show {
        #[arg(help = "Story id whose tasks should be shown, for example US-F1-053.")]
        story_id: String,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Add a story task. Effect: appends a task block to the story's sibling .tasks.md file. Side effects: does not create standalone T-*.md files."
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
        about = "Update a story task. Effect: rewrites the matching task block in the story's sibling .tasks.md file. Side effects: only supplied fields are changed."
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
    #[command(
        about = "Delete a story task. Effect: removes the matching task block from the story's sibling .tasks.md file. Side effects: no standalone task artifacts are created."
    )]
    Delete {
        #[arg(help = "Parent story id for the task, for example US-F1-053.")]
        story_id: String,
        #[arg(help = "Task id to delete, for example TASK-US-F1-053-001.")]
        task_id: String,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

pub(crate) const COMPLETION_HELP: &str = "Generate a shell completion script from the current kanban command tree.\n\nInstall zsh completion — add to ~/.zshrc:\n  eval \"$(kanban completion zsh)\"\n\nInstall bash completion — add to ~/.bashrc or ~/.bash_profile:\n  eval \"$(kanban completion bash)\"\n\nInstall PowerShell completion — add to $PROFILE:\n  kanban completion powershell | Out-String | Invoke-Expression\n\nNote on direnv: .envrc is evaluated as bash, so eval \"$(kanban completion zsh)\" cannot\nbe placed there. Add the eval line to ~/.zshrc instead; it runs once per shell.\n\nSupported shells: bash, zsh, powershell. The command only prints completion scripts and never edits shell config files.";
pub(crate) const DOCTOR_HELP: &str = "Diagnose and optionally fix repository workflow issues.\n\nUsage shortcuts:\n  kanban doctor [REPO_ROOT]        Same as `kanban doctor show [REPO_ROOT]`\n  kanban doctor help               Print this help text\n\nEffects depend on subcommand; `show` is read-only while `fix` rewrites only the affected markdown files.";
pub(crate) const WBS_REPORT_HELP: &str = "\
Emit full WBS report data as JSON for piping into the Python xlsx generator.

The command itself is read-only (no files are written). The xlsx file is produced
by the separate Python script that reads the JSON from stdin.

GENERATE THE EXCEL REPORT
  kanban --format json report wbs \\
    | python3 ../autopass-kanban/scripts/wbs_report.py \\
        --template delivery/backlog/2026-03-31.autopass_ip_2.0_wbs.xlsx \\
        --output   delivery/backlog/wbs_report.xlsx

REQUIRED DEPENDENCIES
  Python 3.9+  and  openpyxl:
    pip3 install openpyxl

ARGUMENTS
  --template PATH   Existing WBS Excel file used as layout/style reference and
                    as the source of Milestone, Period, Priority, and Notes values
                    for rows that appear in the template.
  --output   PATH   Destination .xlsx file (parent directory is created if needed).

OUTPUT WORKBOOK — four sheets:

  WBS – AutoPASS IP 2.0
    Hierarchically numbered (1 / 1.1 / 1.1.1) WBS rebuilt from live backlog data.
    All stories are placed in their correct Phase → Epic position — new or renumbered
    stories are never appended to the end.
    Columns:
      A  WBS No       Hierarchical number (phase.epic.story)
      B  ID           Phase code, EP-*, or US-* identifier
      C  Title        Story / epic / phase title
      D  Milestone    Milestone tag (from template or phase metadata)
      E  Period       Target quarter (from template or phase metadata)
      F  Priority     Priority (from template or phase metadata)
      G  Status       Current workflow status
      H  Story Pts    Story points; SUM formula on epic and phase rows
      I  Est Hours    Estimated hours (story points × hours/point from velocity);
                      shown for all stories including done and in-progress
      J  Start Date   Actual work_started for done/in-progress stories;
                      velocity-based estimate for not-yet-started stories
      K  End Date     Actual work_done for done stories; estimated completion
                      for in-progress and not-yet-started stories
      L  Notes        Carried over from the template for rows that exist there

    Hours per point = (sprint_weeks × 5 days × 7 h/day) ÷ avg_pts_per_sprint
    Date estimates are sequenced in planning order (F1 → F5, then by epic and
    story ID within each phase) starting from today.

  Phase Summary
    One row per phase: story count, epic count, and point totals split by status.

  Sprint Burndown
    Historical sprint velocity and a projected completion prognosis based on
    current average points per sprint.

  Legend & Guide
    Copied verbatim from the template if that sheet exists.

EFFECT
  kanban report wbs   — read-only; no backlog or sprint files are modified.
  wbs_report.py       — writes only the --output file; the template is never modified.";
pub(crate) const BASH_DATE_PLACEHOLDER: &str = "YYYY-MM-DD";

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum CompletionTarget {
    Bash,
    Zsh,
    #[value(name = "powershell")]
    PowerShell,
    Help,
}

impl CompletionTarget {
    pub(crate) fn generator(self) -> Option<clap_complete::Shell> {
        match self {
            CompletionTarget::Bash => Some(clap_complete::Shell::Bash),
            CompletionTarget::Zsh => Some(clap_complete::Shell::Zsh),
            CompletionTarget::PowerShell => Some(clap_complete::Shell::PowerShell),
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum FeatureName {
    Sprints,
    Epics,
    Phases,
}

#[derive(Subcommand)]
pub(crate) enum FeaturesCommand {
    #[command(
        about = "List enabled and disabled optional features. Effect: read-only inspection of `.kanban/settings.json`. Side effects: none."
    )]
    List {
        #[arg(help = "Repository path to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Enable an optional feature. Effect: edits `.kanban/settings.json`. Side effects: re-enables subcommands and validation rules for the feature."
    )]
    Enable {
        #[arg(help = "Feature to enable: sprints, epics, or phases.")]
        feature: FeatureName,
        #[arg(help = "Repository path to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Disable an optional feature. Effect: edits `.kanban/settings.json`. Side effects: hides subcommands and skips validation rules for the feature."
    )]
    Disable {
        #[arg(help = "Feature to disable: sprints, epics, or phases.")]
        feature: FeatureName,
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
        about = "Guide fixes for doctor findings. Effect: rewrites affected markdown files one issue at a time. Side effects: prompts before each fix. Pass --non-interactive to apply all safe automatic fixes without prompting; guided/manual fixes are skipped with a summary."
    )]
    Fix {
        #[arg(help = "Optional scope: a story id like US-F1-053 or the literal `current`.")]
        target: Option<String>,
        #[arg(
            long,
            help = "Do not prompt. Apply every safe automatic fix and skip guided/manual fixes with a summary."
        )]
        non_interactive: bool,
        #[arg(help = "Repository root to update. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum WebCommand {
    #[command(hide = true, about = "Run the embedded kanban web server.")]
    Serve {
        #[arg(long, help = "Repository root to serve.")]
        repo_root: PathBuf,
        #[arg(long, help = "Host address to bind.")]
        host: String,
        #[arg(long, help = "Port to bind.")]
        port: u16,
    },
    #[command(
        about = "Start the local kanban web UI. Effect: launches the embedded Rust web server and writes .kanban/run/web.pid plus .kanban/run/web.log. Side effects: no backlog markdown is modified."
    )]
    Start {
        #[arg(long, help = "Run in the foreground instead of writing a PID file.")]
        foreground: bool,
        #[arg(
            long,
            help = "Open the configured web URL in the default browser after start."
        )]
        open: bool,
        #[arg(
            long,
            help = "Run the Vite frontend development server from `web/`. Use a separate `kanban web serve` process for live API requests."
        )]
        dev: bool,
        #[arg(long, help = "Build `web/` before starting in production mode.")]
        build: bool,
        #[arg(help = "Repository root to serve. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Stop the local kanban web UI. Effect: sends SIGTERM to the recorded web process and escalates to forced termination if needed, then removes stale runtime files. Side effects: no backlog markdown is modified."
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
        #[arg(
            long,
            help = "Run the Vite frontend development server from `web/`. Use a separate `kanban web serve` process for live API requests."
        )]
        dev: bool,
        #[arg(long, help = "Build `web/` before starting in production mode.")]
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
pub(crate) enum ReportCommand {
    #[command(
        about = "Emit WBS report data as JSON for piping into the Python xlsx generator. Effect: read-only. Side effects: none.",
        long_about = WBS_REPORT_HELP
    )]
    Wbs {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    #[command(
        about = "Emit canonical probabilistic completion forecast data. Effect: read-only. Side effects: none."
    )]
    Forecast {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Command {
    #[command(
        about = "Initialize `.kanban` in the repository root. Effect: creates default JSON config files in `.kanban/`. Side effects: no backlog files are modified."
    )]
    Init {
        #[arg(help = "Repository path to initialize. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
        #[arg(
            long,
            help = "Skip the sprint feature. Use this when the repository does not organize work into sprints."
        )]
        no_sprints: bool,
        #[arg(
            long,
            help = "Skip the epic feature. Use this when the repository does not organize work into epics."
        )]
        no_epics: bool,
        #[arg(
            long,
            help = "Skip the phase feature. Use this when the repository does not organize work into phases."
        )]
        no_phases: bool,
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
        about = "Inspect epics. Effect: read-only inspection of canonical epic files and child story progress. Side effects: none."
    )]
    Epic {
        #[command(subcommand)]
        command: EpicCommand,
    },
    #[command(
        about = "Inspect or move user stories. Effects depend on subcommand; write operations mutate canonical story frontmatter and sprint story table markdown."
    )]
    Story {
        #[command(subcommand)]
        command: StoryCommand,
    },
    #[command(
        about = "Maintain story task logs. Effect: mutates sibling .tasks.md files only. Side effects: no standalone task artifacts are created."
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
            help = "Shell to generate completion for, or help for setup instructions. Supported values: bash, zsh, powershell, help."
        )]
        target: CompletionTarget,
    },
    #[command(
        about = "Uninstall kanban files installed by the kanban installer. Effect: removes manifest-tracked files and kanban-installer rc lines.",
        long_about = "Uninstall kanban files installed by the kanban installer.\n\nThis command runs the embedded POSIX uninstaller from the current kanban binary. It removes only manifest-tracked files whose hashes still match the install manifest, strips shell rc lines tagged with `kanban-installer:`, and removes the install manifest directory."
    )]
    Uninstall {
        #[arg(long, help = "Prefix used during install (default: ~/.local/bin).")]
        prefix: Option<PathBuf>,
        #[arg(long, help = "Skills directory used during install (optional).")]
        skills_dir: Option<PathBuf>,
        #[arg(long, help = "Skip confirmation prompts.")]
        yes: bool,
        #[arg(long, help = "Preview all actions without modifying the filesystem.")]
        dry_run: bool,
        #[arg(long, help = "Suppress non-error log lines.")]
        quiet: bool,
    },
    #[command(
        about = "Upgrade kanban using the remote GitHub installer. Effect: downloads and runs the latest release installer.",
        long_about = "Upgrade kanban using the remote GitHub installer.\n\nThis command resolves the latest published release tag, downloads the canonical install script pinned to that tag (never `main`), verifies its SHA-256 against the release's checksum asset, and only then runs it locally. The installer's latest-release resolution, checksum verification, manifest reconciliation, completion refresh, and skill upgrade behavior are preserved. See delivery/decisions/ADR-002 for the trust model."
    )]
    Upgrade {
        #[arg(
            long,
            help = "Install directory for the binary (default: ~/.local/bin)."
        )]
        prefix: Option<PathBuf>,
        #[arg(long, help = "Install agent skills to this directory.")]
        skills_dir: Option<PathBuf>,
        #[arg(long, help = "Skip agent skill installation.")]
        no_skills: bool,
        #[arg(long, help = "Accept installer defaults without prompting.")]
        yes: bool,
        #[arg(long, help = "Skip installer safety prompts.")]
        force: bool,
        #[arg(
            long,
            help = "Preview the remote latest-release install without changing files."
        )]
        dry_run: bool,
        #[arg(long, help = "Suppress non-error installer log lines.")]
        quiet: bool,
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
        about = "Generate reports. Effect: read-only aggregation of stories and sprints. Side effects: none.",
        long_about = "Generate reports from backlog and sprint data.\n\nAvailable reports:\n  wbs   Full WBS data as JSON, piped into the Python xlsx generator.\n        Run `kanban report wbs --help` for the complete xlsx generation guide."
    )]
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    #[command(
        about = "Toggle optional backlog features (phases, sprints, epics). Effect: edits `.kanban/settings.json`. Side effects: changes which subcommands and validation rules apply."
    )]
    Features {
        #[command(subcommand)]
        command: FeaturesCommand,
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
    #[command(
        hide = true,
        about = "List task IDs for shell completion. Effect: read-only listing of task IDs for one story. Side effects: none."
    )]
    ListTaskIds {
        #[arg(help = "Story id whose task IDs should be listed, for example US-F1-053.")]
        story_id: String,
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

pub(crate) fn command_repo_root(command: &Command) -> Option<&PathBuf> {
    match command {
        Command::Init { repo_root, .. }
        | Command::Validate { repo_root }
        | Command::ListIds { repo_root, .. }
        | Command::ListTaskIds { repo_root, .. } => Some(repo_root),
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
        Command::Epic { command } => match command {
            EpicCommand::Show { repo_root, .. } | EpicCommand::Update { repo_root, .. } => {
                Some(repo_root)
            }
        },
        Command::Story { command } => match command {
            StoryCommand::Show { repo_root, .. }
            | StoryCommand::List { repo_root, .. }
            | StoryCommand::Move { repo_root, .. }
            | StoryCommand::Plan { repo_root, .. }
            | StoryCommand::Delete { repo_root, .. }
            | StoryCommand::Update { repo_root, .. } => Some(repo_root),
        },
        Command::Task { command } => match command {
            TaskCommand::Show { repo_root, .. }
            | TaskCommand::Add { repo_root, .. }
            | TaskCommand::Update { repo_root, .. }
            | TaskCommand::Delete { repo_root, .. } => Some(repo_root),
        },
        Command::Web { command } => match command {
            WebCommand::Start { repo_root, .. }
            | WebCommand::Serve { repo_root, .. }
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
        Command::Report { command } => match command {
            ReportCommand::Wbs { repo_root } | ReportCommand::Forecast { repo_root } => {
                Some(repo_root)
            }
        },
        Command::Features { command } => match command {
            FeaturesCommand::List { repo_root }
            | FeaturesCommand::Enable { repo_root, .. }
            | FeaturesCommand::Disable { repo_root, .. } => Some(repo_root),
        },
        Command::Completion { .. } | Command::Uninstall { .. } | Command::Upgrade { .. } => None,
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
        CompletionTarget::PowerShell => "powershell",
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn epic_show_parses_epic_id() {
        let args = Args::try_parse_from(["kanban", "epic", "show", "EP-F1-06"]).unwrap();

        match args.command {
            Command::Epic {
                command: EpicCommand::Show { id, repo_root },
            } => {
                assert_eq!(id, "EP-F1-06");
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn epic_update_parses_priority() {
        let args =
            Args::try_parse_from(["kanban", "epic", "update", "EP-F1-02", "--priority", "10"])
                .unwrap();

        match args.command {
            Command::Epic {
                command:
                    EpicCommand::Update {
                        id,
                        priority,
                        planned_start,
                        planned_end,
                        work_started,
                        work_done,
                        repo_root,
                    },
            } => {
                assert_eq!(id, "EP-F1-02");
                assert_eq!(priority, Some(Some("10".to_string())));
                assert_eq!(planned_start, None);
                assert_eq!(planned_end, None);
                assert_eq!(work_started, None);
                assert_eq!(work_done, None);
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn epic_update_parses_bare_frontmatter_option_as_prompt() {
        let args =
            Args::try_parse_from(["kanban", "epic", "update", "EP-F1-02", "--priority"]).unwrap();

        match args.command {
            Command::Epic {
                command:
                    EpicCommand::Update {
                        priority,
                        planned_start,
                        planned_end,
                        work_started,
                        work_done,
                        ..
                    },
            } => {
                assert_eq!(priority, Some(None));
                assert_eq!(planned_start, None);
                assert_eq!(planned_end, None);
                assert_eq!(work_started, None);
                assert_eq!(work_done, None);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn epic_update_parses_lifecycle_fields() {
        let args = Args::try_parse_from([
            "kanban",
            "epic",
            "update",
            "EP-F1-02",
            "--planned-start",
            "2026-06-15",
            "--planned-end",
            "2026-06-19",
            "--work-started",
            "2026-06-16T09:00:00+0200",
            "--work-done",
            "2026-06-18T17:00:00+0200",
        ])
        .unwrap();

        match args.command {
            Command::Epic {
                command:
                    EpicCommand::Update {
                        planned_start,
                        planned_end,
                        work_started,
                        work_done,
                        ..
                    },
            } => {
                assert_eq!(planned_start, Some(Some("2026-06-15".to_string())));
                assert_eq!(planned_end, Some(Some("2026-06-19".to_string())));
                assert_eq!(
                    work_started,
                    Some(Some("2026-06-16T09:00:00+0200".to_string()))
                );
                assert_eq!(
                    work_done,
                    Some(Some("2026-06-18T17:00:00+0200".to_string()))
                );
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn epic_edit_alias_parses_as_update() {
        let args = Args::try_parse_from(["kanban", "epic", "edit", "EP-F1-02"]).unwrap();

        match args.command {
            Command::Epic {
                command: EpicCommand::Update { id, repo_root, .. },
            } => {
                assert_eq!(id, "EP-F1-02");
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
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
                        priority,
                        status,
                        repo_root,
                        ..
                    },
            } => {
                assert_eq!(id, "US-F1-099");
                assert_eq!(story_points, Some(Some("5".to_string())));
                assert_eq!(priority, None);
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
    fn uninstall_command_parses_self_management_flags() {
        let args = Args::try_parse_from([
            "kanban",
            "uninstall",
            "--prefix",
            "/tmp/bin",
            "--skills-dir",
            "/tmp/skills",
            "--yes",
            "--dry-run",
            "--quiet",
        ])
        .unwrap();

        match args.command {
            Command::Uninstall {
                prefix,
                skills_dir,
                yes,
                dry_run,
                quiet,
            } => {
                assert_eq!(prefix, Some(PathBuf::from("/tmp/bin")));
                assert_eq!(skills_dir, Some(PathBuf::from("/tmp/skills")));
                assert!(yes);
                assert!(dry_run);
                assert!(quiet);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn upgrade_command_parses_remote_installer_flags() {
        let args = Args::try_parse_from([
            "kanban",
            "upgrade",
            "--prefix",
            "/tmp/bin",
            "--no-skills",
            "--yes",
            "--force",
            "--dry-run",
            "--quiet",
        ])
        .unwrap();

        match args.command {
            Command::Upgrade {
                prefix,
                skills_dir,
                no_skills,
                yes,
                force,
                dry_run,
                quiet,
            } => {
                assert_eq!(prefix, Some(PathBuf::from("/tmp/bin")));
                assert_eq!(skills_dir, None);
                assert!(no_skills);
                assert!(yes);
                assert!(force);
                assert!(dry_run);
                assert!(quiet);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn doctor_fix_current_parses() {
        let args = Args::try_parse_from(["kanban", "doctor", "fix", "current"]).unwrap();

        match args.command {
            Command::Doctor {
                command: DoctorCommand::Fix { target, .. },
            } => {
                assert_eq!(target.as_deref(), Some("current"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn doctor_fix_story_parses() {
        let args = Args::try_parse_from(["kanban", "doctor", "fix", "US-F1-053"]).unwrap();

        match args.command {
            Command::Doctor {
                command: DoctorCommand::Fix { target, .. },
            } => {
                assert_eq!(target.as_deref(), Some("US-F1-053"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn doctor_fix_non_interactive_flag_parses() {
        let args =
            Args::try_parse_from(["kanban", "doctor", "fix", "--non-interactive", "current"])
                .unwrap();

        match args.command {
            Command::Doctor {
                command:
                    DoctorCommand::Fix {
                        target,
                        non_interactive,
                        repo_root,
                    },
            } => {
                assert_eq!(target.as_deref(), Some("current"));
                assert!(non_interactive);
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn task_show_parses_story_id() {
        let args = Args::try_parse_from(["kanban", "task", "show", "US-F1-057"]).unwrap();

        match args.command {
            Command::Task {
                command:
                    TaskCommand::Show {
                        story_id,
                        repo_root,
                    },
            } => {
                assert_eq!(story_id, "US-F1-057");
                assert_eq!(repo_root, PathBuf::from("."));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn task_delete_parses_story_and_task_id() {
        let args = Args::try_parse_from([
            "kanban",
            "task",
            "delete",
            "US-F1-057",
            "TASK-US-F1-057-001",
        ])
        .unwrap();

        match args.command {
            Command::Task {
                command:
                    TaskCommand::Delete {
                        story_id,
                        task_id,
                        repo_root,
                    },
            } => {
                assert_eq!(story_id, "US-F1-057");
                assert_eq!(task_id, "TASK-US-F1-057-001");
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
}
