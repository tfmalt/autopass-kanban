use std::path::PathBuf;

use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use kanban_core::{
    CreateSprintInput, DoctorFinding, PhaseOverview, RolloverResult, SprintOverview, StoryDetails,
    StoryKind, TaskSummary, add_task_to_story, create_sprint, doctor_repository, find_story,
    move_story_to_status_with_assignee, rollover_sprint, suggested_next_sprint_dates,
    suggested_next_sprint_number, suggested_sprint_dates, summarize_current_sprint,
    summarize_phase, summarize_sprint, summarize_sprints, update_task_in_story,
    validate_repository,
};

#[derive(Parser)]
#[command(name = "kanban")]
#[command(bin_name = "kanban")]
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
        #[arg(
            help = "Sprint folder name to inspect, for example S001.2026-06-01--2026-06-12.foundation."
        )]
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
}

fn print_sprint_overview(sprint: &SprintOverview) {
    println!("Sprint: {}", sprint.sprint_name);
    println!("Headline: {}", sprint.headline);
    println!("Dates: {} .. {}", sprint.start_date, sprint.end_date);
    println!(
        "README: {}{}",
        sprint.readme_path.display(),
        sprint
            .readme_status
            .as_deref()
            .map(|status| format!(" (status: {status})"))
            .unwrap_or_default()
    );

    if !sprint.warnings.is_empty() {
        println!("Warnings:");
        for warning in &sprint.warnings {
            println!("- {warning}");
        }
    }

    println!("Stories by status:");
    for status in ["todo", "in-progress", "ready-for-qa", "done", "blocked"] {
        let stories = sprint
            .stories_by_status
            .get(status)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        println!("- {status}: {}", stories.len());
        for story in stories {
            let task_suffix = story
                .task_summary
                .as_ref()
                .map(format_task_summary)
                .unwrap_or_default();
            if task_suffix.is_empty() {
                println!("  {} {} [{}]", story.id, story.title, story.assignee);
            } else {
                println!(
                    "  {} {} [{}] {}",
                    story.id, story.title, story.assignee, task_suffix
                );
            }
        }
    }

    println!("Blocked work:");
    if sprint.blocked_work.is_empty() {
        println!("- none");
    } else {
        for item in &sprint.blocked_work {
            match (&item.task_id, &item.task_title) {
                (Some(task_id), Some(task_title)) => {
                    println!(
                        "- {} {} -> {} {}",
                        item.story_id, item.story_title, task_id, task_title
                    );
                }
                _ => println!("- {} {}", item.story_id, item.story_title),
            }
        }
    }
}

fn print_phase_overview(phase: &PhaseOverview) {
    println!("Phase: {}", phase.phase);
    println!("Stories: {}", phase.stories.len());
    for story in &phase.stories {
        let sprint = story.sprint.as_deref().unwrap_or("~");
        println!(
            "- {} [{}] sprint={} assignee={} points={} {}",
            story.id, story.status, sprint, story.assignee, story.story_points, story.title
        );
    }
}

fn print_story_details(details: &StoryDetails) {
    let kind = match details.story.kind {
        StoryKind::Backlog => "backlog",
        StoryKind::Sprint => "sprint",
    };

    println!("Story: {}", details.story.id);
    println!("Title: {}", details.story.title);
    println!("Kind: {kind}");
    println!("Status: {}", details.story.status);
    println!("Assignee: {}", details.story.assignee);
    println!("Story points: {}", details.story.story_points);
    println!("Path: {}", details.story.relative_path.display());

    if let Some(sprint) = &details.story.sprint {
        println!("Sprint: {sprint}");
    }
    if let Some(source_path) = &details.source_story_path {
        println!("Source story: {}", source_path.display());
    }
    if let Some(task_file_path) = &details.task_file_path {
        println!("Task file: {}", task_file_path.display());
    }
    if let Some(summary) = &details.story.task_summary {
        println!("Task summary: {}", format_task_summary(summary));
    }

    print_optional_section("Story Statement", details.story_statement.as_deref());
    print_optional_section(
        "Acceptance Criteria",
        details.acceptance_criteria.as_deref(),
    );
    print_optional_section("Definition Of Done", details.definition_of_done.as_deref());
    print_optional_section(
        "Notes And Open Questions",
        details.notes_and_open_questions.as_deref(),
    );

    println!("Tasks:");
    if details.tasks.is_empty() {
        println!("- none");
    } else {
        for task in &details.tasks {
            println!("- {} [{}] {}", task.id, task.normalized_status, task.title);
        }
    }
}

fn print_optional_section(title: &str, content: Option<&str>) {
    if let Some(content) = content {
        println!("{title}:");
        println!("{content}");
    }
}

fn print_doctor_findings(findings: &[DoctorFinding]) {
    if findings.is_empty() {
        println!("No doctor findings.");
        return;
    }

    for finding in findings {
        println!(
            "{} [{}] {}",
            finding.scope, finding.severity, finding.message
        );
    }
}

fn format_task_summary(summary: &TaskSummary) -> String {
    format!(
        "tasks(todo={}, in-progress={}, blocked={}, done={})",
        summary.todo, summary.in_progress, summary.blocked, summary.done
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

fn print_rollover_result(result: &RolloverResult) {
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
        "Rolled sprint {} -> {}",
        result.from_sprint, result.to_sprint
    );
    println!(
        "Created next sprint: {}",
        if result.created_next_sprint {
            "yes"
        } else {
            "no"
        }
    );
    println!("Completed stories: {completed}");
    println!("Carried stories: {carried}");
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Sprint { command } => match command {
            SprintCommand::Current { repo_root } => {
                let sprint = summarize_current_sprint(repo_root)?;
                print_sprint_overview(&sprint);
            }
            SprintCommand::List { repo_root } => {
                let sprints = summarize_sprints(repo_root)?;
                for sprint in sprints {
                    println!(
                        "- {} [{}..{}]{}",
                        sprint.sprint_name,
                        sprint.start_date,
                        sprint.end_date,
                        sprint
                            .readme_status
                            .as_deref()
                            .map(|status| format!(" README={status}"))
                            .unwrap_or_default()
                    );
                }
            }
            SprintCommand::Show { name, repo_root } => {
                let sprint = summarize_sprint(repo_root, &name)?;
                print_sprint_overview(&sprint);
            }
            SprintCommand::Create { repo_root } => {
                let input = prompt_create_sprint(&repo_root, None, None)?;
                let result = create_sprint(repo_root, &input)?;
                println!("Created sprint: {}", result.sprint_name);
                println!("Path: {}", result.sprint_path.display());
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
                print_rollover_result(&result);
            }
        },
        Command::Phase { command } => match command {
            PhaseCommand::Show { phase, repo_root } => {
                let phase = summarize_phase(repo_root, &phase)?;
                print_phase_overview(&phase);
            }
        },
        Command::Story { command } => match command {
            StoryCommand::Show { id, repo_root } => match find_story(repo_root, &id)? {
                Some(details) => print_story_details(&details),
                None => println!("Story not found: {id}"),
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
                    "Moved {} in {}: {} -> {}",
                    result.story_id, result.sprint_name, result.from_status, result.to_status
                );
                println!("Story: {}", result.story_path.display());
                if let Some(task_path) = result.task_path {
                    println!("Task file: {}", task_path.display());
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
                println!("Added {} to {}", result.task_id, result.story_id);
                println!("Task file: {}", result.task_file_path.display());
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
                println!("Updated {} in {}", result.task_id, result.story_id);
                println!("Task file: {}", result.task_file_path.display());
            }
        },
        Command::Validate { repo_root } => {
            let report = validate_repository(repo_root)?;
            if report.issues.is_empty() {
                println!("No validation issues found.");
            } else {
                for issue in report.issues {
                    println!(
                        "{} [{}] {}",
                        issue.file_path.display(),
                        issue.rule,
                        issue.message
                    );
                }
            }
        }
        Command::Doctor { repo_root } => {
            let findings = doctor_repository(repo_root)?;
            print_doctor_findings(&findings);
        }
    }

    Ok(())
}
