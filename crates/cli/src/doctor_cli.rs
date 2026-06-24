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
        println!("{} no doctor findings.", theme.ok_label());
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
    println!("{} scope: {}", theme.info_label(), issue.scope);
    if let Some(story_id) = &issue.story_id {
        println!("{} story: {}", theme.info_label(), theme.id(story_id));
    }
    if let Some(path) = &issue.file_path {
        println!(
            "{} file: {}",
            theme.info_label(),
            theme.path(path.display())
        );
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
                let theme = Theme::for_stdout(ColorMode::Auto);
                if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
                    println!("{} enter skip or quit.", theme.warning_label());
                } else if doctor_issue_allows_edit(issue) {
                    println!("{} enter yes, edit, skip, or quit.", theme.warning_label());
                } else {
                    println!("{} enter yes, skip, or quit.", theme.warning_label());
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
            let theme = Theme::for_stdout(ColorMode::Auto);
            println!("{} choose one of: {options_text}.", theme.warning_label());
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
        println!("{} no doctor findings to fix.", theme.ok_label());
        return Ok(());
    }

    let mut index = 0;
    while index < issues.len() {
        let total = issues.len();
        let issue = issues[index].clone();
        print_doctor_issue(theme, index + 1, total, &issue);
        if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
            println!(
                "{} manual-only issue; no automatic fix is available.",
                theme.info_label()
            );
        }
        let action = prompt_doctor_fix_action(&issue)?;
        match action.as_str() {
            "q" | "quit" => {
                println!("{} doctor fix aborted.", theme.warning_label());
                return Ok(());
            }
            "s" | "skip" => {
                index += 1;
            }
            "e" | "edit" => {
                let input = collect_doctor_edit_input(&issue)?;
                let result = apply_doctor_fix(repo_root, &issue, &input)?;
                println!("{} applied: {}", theme.ok_label(), result.message);
                for path in result.touched_paths {
                    println!(
                        "{} updated: {}",
                        theme.info_label(),
                        theme.path(path.display())
                    );
                }
                issues = resolve_doctor_fix_issues(repo_root, target)?;
                if issues.is_empty() {
                    println!(
                        "{} all scoped doctor findings are resolved.",
                        theme.ok_label()
                    );
                    return Ok(());
                }
            }
            _ => {
                if matches!(issue.fix_kind, DoctorFixKind::ManualOnly) {
                    println!("{} skipping manual-only issue.", theme.warning_label());
                    index += 1;
                    continue;
                }
                let input = collect_doctor_fix_input(&issue)?;
                let result = apply_doctor_fix(repo_root, &issue, &input)?;
                println!("{} applied: {}", theme.ok_label(), result.message);
                for path in result.touched_paths {
                    println!(
                        "{} updated: {}",
                        theme.info_label(),
                        theme.path(path.display())
                    );
                }
                issues = resolve_doctor_fix_issues(repo_root, target)?;
                if issues.is_empty() {
                    println!(
                        "{} all scoped doctor findings are resolved.",
                        theme.ok_label()
                    );
                    return Ok(());
                }
            }
        }
    }

    println!("{} doctor fix wizard completed.", theme.ok_label());
    Ok(())
}

/// `true` when a doctor issue can be applied without any prompted input: an
/// automatic fix that needs no text/choice value. Used by the non-interactive
/// path (US-011) to decide which fixes to apply unattended.
fn is_auto_applyable(issue: &DoctorIssue) -> bool {
    matches!(issue.fix_kind, DoctorFixKind::Automatic) && matches!(issue.prompt, DoctorPrompt::None)
}

/// Apply every safe automatic doctor fix without prompting and skip guided or
/// manual fixes with a summary (US-011 scenario 4). Re-resolves issues after
/// each apply so dependent findings are not applied against stale state.
pub(crate) fn run_doctor_fix_non_interactive(
    theme: &Theme,
    repo_root: &PathBuf,
    target: Option<&str>,
) -> Result<()> {
    let issues = resolve_doctor_fix_issues(repo_root, target)?;
    if issues.is_empty() {
        println!("{} no doctor findings to fix.", theme.ok_label());
        return Ok(());
    }

    let mut applied = 0usize;
    loop {
        let current = resolve_doctor_fix_issues(repo_root, target)?;
        let Some(position) = current.iter().position(is_auto_applyable) else {
            break;
        };
        let issue = current[position].clone();
        let input = DoctorFixInput { value: None };
        let result = apply_doctor_fix(repo_root, &issue, &input)?;
        println!("{} applied: {}", theme.ok_label(), result.message);
        for path in result.touched_paths {
            println!(
                "{} updated: {}",
                theme.info_label(),
                theme.path(path.display())
            );
        }
        applied += 1;
    }

    let remaining = resolve_doctor_fix_issues(repo_root, target)?;
    println!(
        "{} applied {} automatic fix(es).",
        theme.ok_label(),
        applied
    );
    if remaining.is_empty() {
        if applied > 0 {
            println!(
                "{} all scoped doctor findings are resolved.",
                theme.ok_label()
            );
        }
    } else {
        println!(
            "{} skipped {} guided/manual fix(es) requiring interaction.",
            theme.info_label(),
            remaining.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_fix_preview_renders_key_old_and_new_values() {
        let theme = Theme::plain();
        let issue = DoctorIssue {
            severity: "info".to_string(),
            scope: "story.md".to_string(),
            file_path: None,
            story_id: None,
            sprint_name: None,
            rule: "invalid-timestamp:updated".to_string(),
            message: String::new(),
            suggestion: String::new(),
            fix_preview: Some(kanban_core::DoctorFixPreview {
                field_name: "updated".to_string(),
                old_value: "2026-05-31".to_string(),
                new_value: "2026-05-31T00:00:00+0200".to_string(),
            }),
            fix_kind: DoctorFixKind::Automatic,
            prompt: DoctorPrompt::None,
        };

        assert_eq!(
            format_doctor_fix_preview(&theme, &issue),
            "updated: 2026-05-31 -> 2026-05-31T00:00:00+0200"
        );
    }

    #[test]
    fn doctor_frontmatter_tokens_are_highlighted_with_color_theme() {
        let theme = Theme::color();
        let highlighted = highlight_frontmatter_tokens(
            &theme,
            "Frontmatter field \"updated\" must replace `2026-05-31`.",
        );

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("updated"));
        assert!(highlighted.contains("2026-05-31"));
    }
}
