use std::path::PathBuf;

use anyhow::Result;
use kanban_core::validate_repository;
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
    Validate {
        #[arg(default_value = ".")]
        repo_root: PathBuf,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
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
    }

    Ok(())
}
