#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, doctor_cli::*, json_out::*, layout::*, render::*, theme::*, web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn prompt(message: &str) -> Result<String> {
    use std::io::{self, Write};

    print!("{message}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

pub(crate) fn prompt_with_default(label: &str, default: &str) -> Result<String> {
    let value = prompt(&format!("{label} [{default}]: "))?;
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value)
    }
}

pub(crate) fn story_frontmatter_update_value(
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

pub(crate) fn open_story_markdown_in_editor(path: &Path) -> Result<()> {
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

pub(crate) fn prompt_date(label: &str, default: NaiveDate) -> Result<NaiveDate> {
    loop {
        let input = prompt_with_default(label, &default.format("%Y-%m-%d").to_string())?;
        match NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
            Ok(date) => return Ok(date),
            Err(_) => println!("Enter a date as YYYY-MM-DD."),
        }
    }
}

pub(crate) fn suggested_sprint_defaults(
    repo_root: &PathBuf,
) -> Result<(u32, Option<(NaiveDate, NaiveDate)>)> {
    let config = kanban_core::load_kanban_config(repo_root)?;
    if !config.sprints_path().is_dir() {
        return Ok((0, None));
    }
    Ok((
        suggested_next_sprint_number(repo_root)?,
        suggested_next_sprint_dates(repo_root)?,
    ))
}

pub(crate) fn prompt_create_sprint(
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

pub(crate) fn print_rollover_result(theme: &Theme, result: &RolloverResult) {
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
