use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Result, bail};
use chrono::NaiveDate;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use kanban_core::{
    CreateSprintInput, DoctorFinding, PhaseOverview, RolloverResult, SprintOverview, StoryDetails,
    StoryKind, StoryOverview, TaskSummary, add_task_to_story, create_sprint, doctor_repository,
    find_story, list_epic_ids, list_sprint_names, list_story_completion_items, list_story_ids,
    move_story_to_status_with_assignee, rollover_sprint, suggested_next_sprint_dates,
    suggested_next_sprint_number, suggested_sprint_dates, summarize_current_sprint,
    summarize_phase, summarize_sprint, summarize_sprints, update_task_in_story,
    validate_repository,
};

const MIN_TERMINAL_WIDTH: usize = 80;
const DEFAULT_OUTPUT_WIDTH: usize = 100;

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
    fn for_stdout() -> Self {
        Self {
            color: std::io::stdout().is_terminal()
                && std::env::var_os("NO_COLOR").is_none()
                && std::env::var_os("TERM").is_none_or(|term| term != "dumb"),
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
        about = "List sprint folders. Effect: read-only inspection of doc/backlog/sprints. Side effects: none."
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
        about = "Create a sprint folder. Effect: writes a sprint README and status folders under doc/backlog/sprints. Side effects: prompts for sprint metadata."
    )]
    Create {
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
enum Command {
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
        about = "Diagnose repository workflow issues. Effect: read-only inspection with actionable findings. Side effects: none."
    )]
    Doctor {
        #[arg(help = "Repository root to inspect. Defaults to the current directory.")]
        #[arg(default_value = ".")]
        repo_root: PathBuf,
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

fn print_sprint_overview(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) {
    print!("{}", render_sprint_overview(theme, layout, sprint));
}

fn render_sprint_overview(theme: &Theme, layout: OutputLayout, sprint: &SprintOverview) -> String {
    let mut output = String::new();
    push_line(
        &mut output,
        &theme.heading(format!("Sprint {}", sprint.sprint_name)),
    );
    push_wrapped_label_value(
        &mut output,
        theme,
        "Headline:",
        &sprint.headline,
        layout.width,
    );
    push_wrapped_label_value(
        &mut output,
        theme,
        "Dates:",
        &format!("{} .. {}", sprint.start_date, sprint.end_date),
        layout.width,
    );
    let readme = sprint
        .readme_status
        .as_deref()
        .map(|status| format!("{} (status: {status})", sprint.readme_path.display()))
        .unwrap_or_else(|| sprint.readme_path.display().to_string());
    push_wrapped_label_value(&mut output, theme, "README:", &readme, layout.width);

    if !sprint.warnings.is_empty() {
        push_line(&mut output, &theme.warning("Warnings:"));
        for warning in &sprint.warnings {
            push_wrapped_hanging_line(&mut output, "- ", warning, layout.width, |value| {
                theme.warning(value)
            });
        }
    }

    push_line(&mut output, &theme.heading("Stories by status"));
    for status in ["todo", "in-progress", "ready-for-qa", "done", "blocked"] {
        push_line(&mut output, "");
        let stories = sprint
            .stories_by_status
            .get(status)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        push_line(
            &mut output,
            &format!("{} ({})", theme.status(status), theme.count(stories.len())),
        );
        if stories.is_empty() {
            push_line(&mut output, "  - none");
        } else {
            push_story_table(&mut output, theme, layout.width, stories);
        }
    }

    push_line(&mut output, &theme.heading("Blocked work"));
    if sprint.blocked_work.is_empty() {
        push_line(&mut output, "- none");
    } else {
        push_blocked_work_table(&mut output, theme, layout.width, &sprint.blocked_work);
    }

    output
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
    Path,
    Warning,
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
                TableCell::styled(&story.id, CellStyle::Id),
                TableCell::new(&story.title),
                TableCell::new(&story.assignee),
                TableCell::styled(
                    format_compact_task_summary(story.task_summary.as_ref()),
                    CellStyle::Path,
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
        .map(|story| display_width(&story.id))
        .max()
        .unwrap_or(5)
        .clamp(5, 12);
    let task_width = stories
        .iter()
        .map(|story| display_width(&format_compact_task_summary(story.task_summary.as_ref())))
        .max()
        .unwrap_or(5)
        .clamp(5, 17);
    let assignee_width = stories
        .iter()
        .flat_map(|story| story.assignee.split_whitespace())
        .map(display_width)
        .max()
        .unwrap_or(8)
        .max(8);
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
        Some(CellStyle::Path) => theme.path(value),
        Some(CellStyle::Warning) => theme.warning(value),
        None => value.to_string(),
    }
}

fn pad_to_width(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(display_width(value));
    format!("{value}{}", " ".repeat(padding))
}

fn display_width(value: &str) -> usize {
    value.chars().count()
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

fn format_compact_task_summary(summary: Option<&TaskSummary>) -> String {
    summary
        .map(|summary| {
            format!(
                "T:{} IP:{} B:{} D:{}",
                summary.todo, summary.in_progress, summary.blocked, summary.done
            )
        })
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

fn format_task_summary(summary: &TaskSummary) -> String {
    format!(
        "tasks(todo={}, in-progress={}, blocked={}, done={})",
        summary.todo, summary.in_progress, summary.blocked, summary.done
    )
}

/// ZSH helper functions appended after the clap_complete-generated script.
/// These provide dynamic completion for sprint names, story IDs, and epic IDs.
const ZSH_DYNAMIC_HELPERS: &str = r#"
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
/// sprint name and story ID arguments with dynamic lookup helpers.
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

/// Enhance the bash completion script with dynamic sprint name and story ID completions.
fn enhance_bash_completion(script: &str) -> String {
    let script = inject_bash_dynamic(
        script,
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
    inject_bash_dynamic(
        &script,
        "kanban__subcmd__task__subcmd__update",
        "-h --title --status --tags --description --help <STORY_ID> <TASK_ID> [REPO_ROOT]",
        "stories",
        3,
    )
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

fn prompt_create_sprint(
    repo_root: &PathBuf,
    suggested_start: Option<NaiveDate>,
    suggested_end: Option<NaiveDate>,
) -> Result<CreateSprintInput> {
    let suggested_number = suggested_next_sprint_number(repo_root)?;
    let number = loop {
        let value = prompt_with_default("Sprint number", &format!("{suggested_number}"))?;
        match value.parse::<u32>() {
            Ok(number) => break number,
            Err(_) => println!("Enter a numeric sprint number."),
        }
    };
    let today = chrono::Local::now().date_naive();
    let repo_suggestion = suggested_next_sprint_dates(repo_root)?;
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

fn main() -> Result<()> {
    let args = Args::parse();
    let theme = Theme::for_stdout();

    match args.command {
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
            SprintCommand::Create { repo_root } => {
                let input = prompt_create_sprint(&repo_root, None, None)?;
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
        Command::Doctor { repo_root } => {
            let findings = doctor_repository(repo_root)?;
            print_doctor_findings(&theme, &findings);
        }
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
            headline: "Terminal wrapping".to_string(),
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

        assert!(output.contains("US-F1-999"));
        assert!(!output.contains('|'));
        for line in output.lines() {
            assert!(
                display_width(line) <= 80,
                "line exceeded 80 columns: {line}"
            );
        }
    }
}
