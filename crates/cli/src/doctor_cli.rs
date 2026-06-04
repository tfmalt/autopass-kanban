#[allow(unused_imports)]
use crate::prelude::*;
#[allow(unused_imports)]
use crate::{
    cli::*, completion::*, json_out::*, layout::*, prompt::*, render::*, theme::*, web::*,
};
#[allow(unused_imports)]
use kanban_core::*;

pub(crate) fn print_doctor_findings(theme: &Theme, findings: &[DoctorFinding]) {
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

pub(crate) fn highlight_frontmatter_tokens(theme: &Theme, text: &str) -> String {
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

pub(crate) fn format_doctor_rule(theme: &Theme, rule: &str) -> String {
    if let Some((prefix, field_name)) = rule.rsplit_once(':') {
        format!("{prefix}:{}", theme.highlight(field_name))
    } else {
        rule.to_string()
    }
}

pub(crate) fn format_doctor_fix_preview(theme: &Theme, issue: &DoctorIssue) -> String {
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

pub(crate) fn print_doctor_issue(theme: &Theme, index: usize, total: usize, issue: &DoctorIssue) {
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

pub(crate) fn resolve_doctor_fix_issues(
    repo_root: &PathBuf,
    target: Option<&str>,
) -> Result<Vec<DoctorIssue>> {
    match target.map(str::trim).filter(|value| !value.is_empty()) {
        None => collect_doctor_issues(repo_root),
        Some("current") => collect_doctor_issues_for_current_sprint(repo_root),
        Some(story_id) => collect_doctor_issues_for_story(repo_root, story_id),
    }
}

pub(crate) fn doctor_issue_allows_edit(issue: &DoctorIssue) -> bool {
    !matches!(issue.fix_kind, DoctorFixKind::ManualOnly)
        && (issue.fix_preview.is_some() || !matches!(issue.prompt, DoctorPrompt::None))
}

pub(crate) fn prompt_doctor_fix_action(issue: &DoctorIssue) -> Result<String> {
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

pub(crate) fn collect_doctor_fix_input(issue: &DoctorIssue) -> Result<DoctorFixInput> {
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

pub(crate) fn collect_doctor_edit_input(issue: &DoctorIssue) -> Result<DoctorFixInput> {
    if let Some(preview) = &issue.fix_preview {
        let value =
            prompt_with_default(&format!("{} value", preview.field_name), &preview.new_value)?;
        return Ok(DoctorFixInput { value: Some(value) });
    }

    collect_doctor_fix_input(issue)
}

pub(crate) fn run_doctor_fix_wizard(
    theme: &Theme,
    repo_root: &PathBuf,
    target: Option<&str>,
) -> Result<()> {
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
