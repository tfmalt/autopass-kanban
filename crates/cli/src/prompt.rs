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
    let stdin = io::stdin();
    let mut lock = stdin.lock();
    read_prompted_line(&mut lock)
}

/// Read one trimmed line from `input`. Returns an error when the stream is at
/// EOF (`read_line` returns `0` bytes) so interactive prompts cannot silently
/// fall back to a default or busy-loop on closed stdin (US-011).
///
/// Note: we deliberately do not bail merely because stdin is not a TTY — piping
/// a real response (`echo "y" | kanban doctor fix`) stays supported. EOF is the
/// precise gate that the acceptance criteria require.
fn read_prompted_line(input: &mut dyn std::io::BufRead) -> Result<String> {
    let mut line = String::new();
    let read = input.read_line(&mut line)?;
    if read == 0 {
        bail!(
            "standard input is closed; cannot prompt for confirmation. \
             Re-run in an interactive terminal or pass --non-interactive."
        );
    }
    Ok(line.trim().to_string())
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

pub(crate) fn open_markdown_in_editor(path: &Path, label: &str) -> Result<()> {
    let editor =
        std::env::var("EDITOR").with_context(|| format!("$EDITOR must be set to edit {label}."))?;
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

pub(crate) fn open_story_markdown_in_editor(path: &Path) -> Result<()> {
    open_markdown_in_editor(path, "story markdown")
}

pub(crate) fn prompt_date(label: &str, default: NaiveDate) -> Result<NaiveDate> {
    loop {
        let input = prompt_with_default(label, &default.format("%Y-%m-%d").to_string())?;
        match NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
            Ok(date) => return Ok(date),
            Err(_) => {
                let theme = Theme::for_stdout(ColorMode::Auto);
                println!("{} enter a date as YYYY-MM-DD.", theme.warning_label());
            }
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
            Err(_) => {
                let theme = Theme::for_stdout(ColorMode::Auto);
                println!("{} enter a numeric sprint number.", theme.warning_label());
            }
        }
    };
    let today = chrono::Local::now().date_naive();
    let default_start = suggested_start
        .or_else(|| repo_suggestion.map(|(start_date, _)| start_date))
        .unwrap_or(today);
    let start_date = loop {
        let date = prompt_date("Start date", default_start)?;
        if date < today {
            let theme = Theme::for_stdout(ColorMode::Auto);
            println!(
                "{} start date cannot be in the past.",
                theme.warning_label()
            );
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
            let theme = Theme::for_stdout(ColorMode::Auto);
            println!(
                "{} end date must be after start date.",
                theme.warning_label()
            );
            continue;
        }
        break date;
    };
    let headline = prompt("Sprint headline: ")?;
    if headline.trim().is_empty() {
        bail!("Sprint headline must not be empty; re-run with a non-empty headline.");
    }
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
        "{} rolled sprint {} -> {}",
        theme.ok_label(),
        result.from_sprint,
        result.to_sprint
    );
    println!(
        "{} created next sprint: {}",
        theme.info_label(),
        if result.created_next_sprint {
            theme.success("yes")
        } else {
            "no".to_string()
        }
    );
    println!("{} completed stories: {completed}", theme.info_label());
    println!("{} carried stories: {carried}", theme.info_label());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_prompted_line_errors_on_eof_with_actionable_message() {
        let mut empty = std::io::Cursor::new(Vec::<u8>::new());
        let result = read_prompted_line(&mut empty);
        assert!(result.is_err(), "EOF must produce an error, not a default");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("standard input is closed"),
            "expected actionable EOF message, got: {msg}"
        );
    }

    #[test]
    fn read_prompted_line_returns_trimmed_value_when_input_present() {
        let mut input = std::io::Cursor::new(b"  yes  \n".to_vec());
        let value = read_prompted_line(&mut input).unwrap();
        assert_eq!(value, "yes");
    }

    #[test]
    fn read_prompted_line_treats_blank_line_as_empty_not_eof() {
        // A real blank line (newline byte) is valid input, not EOF, so the
        // doctor wizard can still interpret it as the default "y" when a human
        // presses enter on a TTY.
        let mut input = std::io::Cursor::new(b"\n".to_vec());
        let value = read_prompted_line(&mut input).unwrap();
        assert_eq!(value, "");
    }
}
