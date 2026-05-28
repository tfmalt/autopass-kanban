use std::path::PathBuf;

use anyhow::Result;
use kanban_core::{
    DoctorFinding, PhaseOverview, SprintOverview, StoryDetails, StoryKind, TaskSummary,
    doctor_repository, find_story, summarize_current_sprint, summarize_phase, summarize_sprint,
    summarize_sprints, validate_repository,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kanban")]
#[command(bin_name = "kanban")]
#[command(visible_alias = "kb")]
#[command(about = "Read-only kanban tooling")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Sprint {
        #[command(subcommand)]
        command: SprintCommand,
    },
    Phase {
        #[command(subcommand)]
        command: PhaseCommand,
    },
    Story {
        #[command(subcommand)]
        command: StoryCommand,
    },
    Validate {
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    Doctor {
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum SprintCommand {
    Current {
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    List {
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
    Show {
        name: String,
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum PhaseCommand {
    Show {
        phase: String,
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum StoryCommand {
    Show {
        id: String,
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
                    println!("- {} {} -> {} {}", item.story_id, item.story_title, task_id, task_title);
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
    print_optional_section("Acceptance Criteria", details.acceptance_criteria.as_deref());
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
        println!("{} [{}] {}", finding.scope, finding.severity, finding.message);
    }
}

fn format_task_summary(summary: &TaskSummary) -> String {
    format!(
        "tasks(todo={}, in-progress={}, blocked={}, done={})",
        summary.todo, summary.in_progress, summary.blocked, summary.done
    )
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
