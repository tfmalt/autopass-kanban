use crate::config::*;
use crate::constants::*;
use crate::doctor::*;
use crate::error::KanbanError;
use crate::markdown::*;
use crate::model::*;
#[allow(unused_imports)]
use crate::prelude::*;
use crate::repository::*;
use crate::story::*;
use crate::util::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SprintFolderSpec {
    pub(crate) sprint_name: String,
    pub(crate) headline: String,
    pub(crate) sprint_goal: Option<String>,
    pub(crate) start_date: NaiveDate,
    pub(crate) end_date: NaiveDate,
    pub(crate) readme_path: PathBuf,
    pub(crate) readme_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SprintReadmeInfo {
    pub(crate) sprint: Option<String>,
    pub(crate) headline: Option<String>,
    pub(crate) sprint_goal: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) start_date: Option<NaiveDate>,
    pub(crate) end_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SprintRosterEntry {
    pub(crate) story: StoryOverview,
    pub(crate) link_path: PathBuf,
}

const LEGACY_ROSTER_HEADING: &str = "## Stories (generated — do not edit)";

pub fn summarize_sprints(repo_root: impl AsRef<Path>) -> Result<Vec<SprintOverview>> {
    let repository = read_repository(repo_root)?;
    summarize_sprints_from_repository(&repository)
}

pub fn summarize_current_sprint(repo_root: impl AsRef<Path>) -> Result<SprintOverview> {
    summarize_current_sprint_at_date(repo_root, Local::now().date_naive())
}

pub fn summarize_current_sprint_at_date(
    repo_root: impl AsRef<Path>,
    today: NaiveDate,
) -> Result<SprintOverview> {
    let repository = read_repository(repo_root)?;
    let sprints = summarize_sprints_from_repository(&repository)?;
    select_current_sprint(&sprints, today)
}

pub fn summarize_sprint(repo_root: impl AsRef<Path>, sprint_name: &str) -> Result<SprintOverview> {
    let repository = read_repository(repo_root)?;
    let sprints = summarize_sprints_from_repository(&repository)?;
    sprints
        .into_iter()
        .find(|sprint| sprint.sprint_name == sprint_name)
        .ok_or_else(|| KanbanError::sprint_not_found(sprint_name).into())
}

pub fn list_current_sprint_stories(
    repo_root: impl AsRef<Path>,
) -> Result<(String, Vec<StoryOverview>)> {
    let sprint = summarize_current_sprint(repo_root)?;
    let sprint_name = sprint.sprint_name.clone();
    Ok((sprint_name, flatten_sprint_stories(&sprint)))
}

pub fn list_next_sprint_stories(
    repo_root: impl AsRef<Path>,
) -> Result<(String, Vec<StoryOverview>)> {
    let repository = read_repository(repo_root)?;
    let sprints = summarize_sprints_from_repository(&repository)?;
    let current = select_current_sprint(&sprints, Local::now().date_naive())?;
    let current_number = parse_sprint_number(&current.sprint_name).ok_or_else(|| {
        anyhow!(
            "Current sprint name does not use the expected SNNN.headline format: {}",
            current.sprint_name
        )
    })?;

    let next = sprints
        .into_iter()
        .filter_map(|sprint| {
            parse_sprint_number(&sprint.sprint_name)
                .filter(|number| *number > current_number)
                .map(|number| (number, sprint))
        })
        .min_by_key(|(number, _)| *number)
        .map(|(_, sprint)| sprint)
        .ok_or_else(|| anyhow!("No later sprint exists after {}.", current.sprint_name))?;

    let sprint_name = next.sprint_name.clone();
    Ok((sprint_name, flatten_sprint_stories(&next)))
}

pub fn list_stories_in_sprint(
    repo_root: impl AsRef<Path>,
    sprint_name: &str,
) -> Result<Vec<StoryOverview>> {
    let sprint = summarize_sprint(repo_root, sprint_name)?;
    Ok(flatten_sprint_stories(&sprint))
}

pub fn ensure_sprints_enabled_for_repo(repo_root: impl AsRef<Path>) -> Result<()> {
    let config = load_kanban_config(repo_root)?;
    if !config.features().sprints {
        bail!(
            "Sprints are disabled in .kanban/settings.json. Run `kanban features enable sprints` to re-enable them."
        );
    }
    Ok(())
}

pub fn suggested_next_sprint_number(repo_root: impl AsRef<Path>) -> Result<u32> {
    let config = load_kanban_config(repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    Ok(specs
        .iter()
        .filter_map(|spec| parse_sprint_number(&spec.sprint_name))
        .max()
        .map(|value| value + 1)
        .unwrap_or(0))
}

pub fn suggested_next_sprint_dates(
    repo_root: impl AsRef<Path>,
) -> Result<Option<(NaiveDate, NaiveDate)>> {
    let config = load_kanban_config(repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    let previous_end_date = specs
        .iter()
        .filter_map(|spec| {
            parse_sprint_number(&spec.sprint_name).map(|number| (number, spec.end_date))
        })
        .max_by_key(|(number, _)| *number)
        .map(|(_, end_date)| end_date);

    Ok(previous_end_date.map(suggested_sprint_dates))
}

pub fn suggested_sprint_dates(previous_end_date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start_date = first_weekday_after(previous_end_date, Weekday::Mon);
    let end_date = first_weekday_on_or_after(start_date + Days::new(11), Weekday::Fri);
    (start_date, end_date)
}

pub fn create_sprint(
    repo_root: impl AsRef<Path>,
    input: &CreateSprintInput,
) -> Result<CreateSprintResult> {
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root.clone();
    let today = Local::now().date_naive();
    if input.start_date < today {
        bail!(
            "Sprint start date {} cannot be in the past relative to {}.",
            input.start_date.format("%Y-%m-%d"),
            today.format("%Y-%m-%d")
        );
    }
    if input.end_date <= input.start_date {
        bail!(
            "Sprint end date {} must be after start date {}.",
            input.end_date.format("%Y-%m-%d"),
            input.start_date.format("%Y-%m-%d")
        );
    }

    let headline = slugify_headline(&input.headline);
    if headline.is_empty() {
        bail!("Sprint headline must contain at least one ASCII letter or number.");
    }

    let sprint_id = format!("S{:03}", input.number);
    let sprint_name = format!("{sprint_id}.{headline}");
    let sprint_file = config.sprints_path().join(format!("{sprint_name}.md"));
    if sprint_file.exists() {
        bail!("Sprint already exists: {sprint_name}");
    }

    fs::create_dir_all(config.sprints_path())
        .with_context(|| format!("create sprints dir {}", config.sprints_path().display()))?;
    let content =
        render_sprint_file_template(&sprint_id, &headline, input.start_date, input.end_date);
    atomic_write(&sprint_file, &content)
        .with_context(|| format!("write sprint file {}", sprint_file.display()))?;

    Ok(CreateSprintResult {
        sprint_name,
        sprint_path: relative_path(&repo_root, &sprint_file),
    })
}

pub fn rollover_sprint(
    repo_root: impl AsRef<Path>,
    sprint_name: &str,
    next_sprint: Option<&CreateSprintInput>,
) -> Result<RolloverResult> {
    let config = load_kanban_config(repo_root)?;
    let repo_root = config.repo_root.clone();
    let repository = read_repository(&repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    let current_spec = specs
        .iter()
        .find(|spec| spec.sprint_name == sprint_name)
        .ok_or_else(|| KanbanError::sprint_not_found(sprint_name))?;

    let expected_next_number = parse_sprint_number(sprint_name).map(|value| value + 1);
    let mut next_sprint_name = specs
        .iter()
        .find(|spec| parse_sprint_number(&spec.sprint_name) == expected_next_number)
        .map(|spec| spec.sprint_name.clone());
    let mut created_next_sprint = false;

    if next_sprint_name.is_none() {
        let input = next_sprint.ok_or_else(|| {
            anyhow!(
                "Next sprint is missing after {sprint_name}. Create it first or provide create input."
            )
        })?;
        let create_result = create_sprint(&repo_root, input)?;
        created_next_sprint = true;
        next_sprint_name = Some(create_result.sprint_name);
    }

    let next_sprint_name = next_sprint_name.ok_or_else(|| anyhow!("Next sprint is missing."))?;
    let mut completed_story_ids = Vec::new();
    let mut carried_story_ids = Vec::new();

    for story in repository
        .stories
        .iter()
        .filter(|story| story.frontmatter.get("sprint").map(String::as_str) == Some(sprint_name))
    {
        let story_id = story.frontmatter.get("id").cloned().unwrap_or_default();
        let status = story.frontmatter.get("status").cloned().unwrap_or_default();
        if status == "done" {
            completed_story_ids.push(story_id);
            continue;
        }

        let now = current_timestamp_string();
        let moved_story_markdown = update_story_frontmatter_markdown(
            &story.markdown,
            &[
                ("sprint", Some(next_sprint_name.clone())),
                ("updated", Some(now.clone())),
                (
                    "work_started",
                    story.frontmatter.get("work_started").cloned(),
                ),
            ],
        )?;
        atomic_write(&story.file_path, &moved_story_markdown).with_context(|| {
            format!("rewrite rolled sprint story {}", story.file_path.display())
        })?;

        carried_story_ids.push(story_id);
    }

    let closed_readme_path = current_spec.readme_path.clone();
    let closed_readme = fs::read_to_string(&closed_readme_path)
        .with_context(|| format!("read sprint summary {}", closed_readme_path.display()))?;
    let closed_readme = update_sprint_summary_for_rollover(
        &closed_readme,
        sprint_name,
        &next_sprint_name,
        &completed_story_ids,
        &carried_story_ids,
    );
    atomic_write(&closed_readme_path, &closed_readme)
        .with_context(|| format!("write sprint summary {}", closed_readme_path.display()))?;
    regenerate_sprint_roster(&config, sprint_name)?;
    regenerate_sprint_roster(&config, &next_sprint_name)?;

    Ok(RolloverResult {
        from_sprint: sprint_name.to_string(),
        to_sprint: next_sprint_name,
        created_next_sprint,
        completed_story_ids,
        carried_story_ids,
    })
}

pub(crate) fn summarize_sprints_from_repository(
    repository: &Repository,
) -> Result<Vec<SprintOverview>> {
    let today = Local::now().date_naive();
    let config = load_kanban_config(&repository.repo_root)?;
    let specs = discover_sprint_folder_specs(&config)?;
    let mut sprints = specs
        .iter()
        .map(|spec| sprint_overview_from_spec(repository, spec, today))
        .collect::<Vec<_>>();
    sprints.sort_by(|left, right| left.sprint_name.cmp(&right.sprint_name));
    Ok(sprints)
}

pub(crate) fn flatten_sprint_stories(sprint: &SprintOverview) -> Vec<StoryOverview> {
    let mut stories = Vec::new();
    let mut seen_statuses = BTreeSet::new();

    for status in SPRINT_STATUS_DISPLAY_ORDER {
        seen_statuses.insert(status);
        if let Some(items) = sprint.stories_by_status.get(status) {
            stories.extend(items.iter().cloned());
        }
    }

    for (status, items) in &sprint.stories_by_status {
        if !seen_statuses.contains(status.as_str()) {
            stories.extend(items.iter().cloned());
        }
    }

    stories
}

pub(crate) fn sprint_overview_from_spec(
    repository: &Repository,
    spec: &SprintFolderSpec,
    today: NaiveDate,
) -> SprintOverview {
    let mut stories_by_status = SPRINT_STATUS_DISPLAY_ORDER
        .iter()
        .map(|status| (status.to_string(), Vec::new()))
        .collect::<BTreeMap<_, _>>();

    let mut blocked_work = Vec::new();

    for story in repository.stories.iter().filter(|story| {
        story.frontmatter.get("sprint").map(String::as_str) == Some(spec.sprint_name.as_str())
    }) {
        let overview = story_overview(&repository.repo_root, story);
        let status_key = normalize_status_alias(&overview.status);
        stories_by_status
            .entry(status_key)
            .or_default()
            .push(overview.clone());

        if overview.status == "blocked" {
            blocked_work.push(BlockedWorkItem {
                story_id: overview.id.clone(),
                story_title: overview.title.clone(),
                task_id: None,
                task_title: None,
            });
        }

        if let Some(task_file) = &story.task_file {
            for task in task_file
                .tasks
                .iter()
                .filter(|task| task.normalized_status == "blocked")
            {
                blocked_work.push(BlockedWorkItem {
                    story_id: overview.id.clone(),
                    story_title: overview.title.clone(),
                    task_id: Some(task.id.clone()),
                    task_title: Some(task.title.clone()),
                });
            }
        }
    }

    for stories in stories_by_status.values_mut() {
        stories.sort_by(|left, right| left.id.cmp(&right.id));
    }

    SprintOverview {
        sprint_name: spec.sprint_name.clone(),
        headline: spec.headline.clone(),
        sprint_goal: spec.sprint_goal.clone(),
        start_date: spec.start_date.format("%Y-%m-%d").to_string(),
        end_date: spec.end_date.format("%Y-%m-%d").to_string(),
        readme_path: relative_path(&repository.repo_root, &spec.readme_path),
        readme_status: spec.readme_status.clone(),
        stories_by_status,
        blocked_work,
        warnings: sprint_warnings(&repository.repo_root, repository, spec, today),
    }
}

pub(crate) fn select_current_sprint(
    sprints: &[SprintOverview],
    today: NaiveDate,
) -> Result<SprintOverview> {
    let current_sprints = sprints
        .iter()
        .filter(|sprint| {
            let start_date = NaiveDate::parse_from_str(&sprint.start_date, "%Y-%m-%d").ok();
            let end_date = NaiveDate::parse_from_str(&sprint.end_date, "%Y-%m-%d").ok();
            match (start_date, end_date) {
                (Some(start_date), Some(end_date)) => date_in_range(today, start_date, end_date),
                _ => false,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    let active_readmes = sprints
        .iter()
        .filter(|sprint| sprint.readme_status.as_deref() == Some("active"))
        .cloned()
        .collect::<Vec<_>>();

    match current_sprints.as_slice() {
        [current] => Ok(current.clone()),
        [] => match active_readmes.as_slice() {
            [current] => Ok(current.clone()),
            [] => Err(anyhow!(
                "No sprint folder date range includes {}.",
                today.format("%Y-%m-%d")
            )),
            _ => Err(anyhow!(
                "No sprint folder date range includes {} and multiple sprint READMEs are marked active: {}. Run `kanban doctor` to inspect the mismatch.",
                today.format("%Y-%m-%d"),
                active_readmes
                    .iter()
                    .map(|sprint| sprint.sprint_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        },
        _ => Err(anyhow!(
            "Multiple sprint folders include {}: {}. Run `kanban doctor` to inspect the overlap.",
            today.format("%Y-%m-%d"),
            current_sprints
                .iter()
                .map(|sprint| sprint.sprint_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

pub(crate) fn discover_sprint_folder_specs(config: &KanbanConfig) -> Result<Vec<SprintFolderSpec>> {
    let sprints_root = config.sprints_path();
    let mut specs = Vec::new();

    for entry in fs::read_dir(&sprints_root)
        .with_context(|| format!("read sprint root {}", sprints_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
        else {
            continue;
        };

        let Some((sprint_id, file_headline)) = parse_sprint_file_name(&file_name) else {
            continue;
        };

        let readme_path = path.clone();
        let readme = parse_sprint_readme(&readme_path)?;
        let start_date = readme.start_date.ok_or_else(|| {
            anyhow!(
                "Sprint README is missing start_date: {}",
                readme_path.display()
            )
        })?;
        let end_date = readme.end_date.ok_or_else(|| {
            anyhow!(
                "Sprint README is missing end_date: {}",
                readme_path.display()
            )
        })?;
        let headline = readme.headline.clone().unwrap_or(file_headline);
        if readme.sprint.as_deref() != Some(sprint_id.as_str()) {
            bail!(
                "Sprint README field `sprint` must match folder sprint id {sprint_id}: {}",
                readme_path.display()
            );
        }

        specs.push(SprintFolderSpec {
            sprint_name: file_name.trim_end_matches(".md").to_string(),
            headline,
            sprint_goal: readme.sprint_goal,
            start_date,
            end_date,
            readme_path,
            readme_status: readme.status,
        });
    }

    specs.sort_by(|left, right| left.sprint_name.cmp(&right.sprint_name));
    Ok(specs)
}

pub(crate) fn parse_sprint_readme(readme_path: &Path) -> Result<SprintReadmeInfo> {
    let markdown = fs::read_to_string(readme_path)
        .with_context(|| format!("read sprint summary {}", readme_path.display()))?;
    let parsed = parse_frontmatter(&markdown);
    Ok(SprintReadmeInfo {
        sprint: parsed.frontmatter.get("sprint").cloned(),
        headline: parsed.frontmatter.get("headline").cloned(),
        sprint_goal: extract_markdown_section(&parsed.body, "Sprint Goal"),
        status: parsed.frontmatter.get("status").cloned(),
        start_date: parsed
            .frontmatter
            .get("start_date")
            .and_then(|value| parse_markdown_date(value)),
        end_date: parsed
            .frontmatter
            .get("end_date")
            .and_then(|value| parse_markdown_date(value)),
    })
}

pub(crate) fn readme_table_value(markdown: &str, key: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        if !line.starts_with('|') {
            return None;
        }

        let parts = line
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() != 2 || parts[0] != key {
            return None;
        }

        Some(parts[1].trim_matches('`').to_string())
    })
}

pub(crate) fn parse_sprint_file_name(file_name: &str) -> Option<(String, String)> {
    let pattern = Regex::new(SPRINT_FILE_PATTERN).expect("valid sprint file regex");
    let captures = pattern.captures(file_name)?;
    let sprint_id = captures.get(1)?.as_str().to_string();
    let headline = captures.get(2)?.as_str().to_string();
    Some((sprint_id, headline))
}

pub(crate) fn parse_sprint_number(sprint_name: &str) -> Option<u32> {
    let prefix = sprint_name.strip_prefix('S')?;
    let number = prefix.split_once('.')?.0;
    number.parse().ok()
}

pub(crate) fn render_sprint_file_template(
    sprint_id: &str,
    headline: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> String {
    format!(
        "---\nsprint: {sprint_id}\nheadline: {headline}\nstart_date: {}\nend_date: {}\nstatus: planned\nwip_limit: null\n---\n\n# {sprint_id}: {headline}\n\n## Sprint Goal\n\nTBD\n\n## Notes For Review / Demo\n\n- Sprint created by `kanban sprint create`.\n\n## End-Of-Sprint Summary\n\nSprint not started yet.\n\n## Expected Carry-Over / Unfinished Stories\n\nNot determined yet.\n\n{}\n",
        start_date.format("%Y-%m-%d"),
        end_date.format("%Y-%m-%d"),
        render_sprint_roster(&[]).trim_end()
    )
}

pub(crate) fn render_sprint_roster(rows: &[SprintRosterEntry]) -> String {
    let mut out = String::new();
    push_line(&mut out, ROSTER_HEADING);
    push_line(&mut out, "");

    render_sprint_roster_summary(&mut out, rows);
    push_line(&mut out, "");

    let mut rows_by_status = BTreeMap::<String, Vec<&SprintRosterEntry>>::new();
    for row in rows {
        rows_by_status
            .entry(row.story.status.clone())
            .or_default()
            .push(row);
    }

    for status in SPRINT_STATUS_DISPLAY_ORDER {
        let mut items = rows_by_status.remove(status).unwrap_or_default();
        items.sort_by(|left, right| left.story.id.cmp(&right.story.id));
        render_sprint_roster_section(&mut out, status, &items);
    }

    for (status, mut items) in rows_by_status {
        items.sort_by(|left, right| left.story.id.cmp(&right.story.id));
        render_sprint_roster_section(&mut out, &status, &items);
    }

    out.trim_end().to_string()
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn render_sprint_roster_section(output: &mut String, status: &str, rows: &[&SprintRosterEntry]) {
    push_line(output, &format!("### {status}"));
    push_line(output, "");

    push_line(output, "| Story | Points | Assignee | Tasks |");
    push_line(output, "|-------|-------:|----------|-------|");

    if rows.is_empty() {
        push_line(output, "| — | — | — | — |");
        push_line(output, "");
        return;
    }

    for row in rows {
        let points = story_points_value(&row.story);
        let assignee = render_assignee_cell(&row.story.assignee);
        let tasks = format_task_summary(row.story.task_summary.as_ref());
        push_line(
            output,
            &format!(
                "| {} | {points} | {assignee} | {tasks} |",
                sprint_story_link_label(row)
            ),
        );
    }

    push_line(output, "");
}

fn render_sprint_roster_summary(output: &mut String, rows: &[SprintRosterEntry]) {
    let mut rows_by_status = BTreeMap::<String, Vec<&SprintRosterEntry>>::new();
    for row in rows {
        rows_by_status
            .entry(row.story.status.clone())
            .or_default()
            .push(row);
    }

    let total_points = rows
        .iter()
        .map(|row| story_points_value(&row.story))
        .sum::<usize>();
    push_line(output, "| Metric | Stories | Points |");
    push_line(output, "|--------|--------:|------:|");
    push_line(
        output,
        &format!("| Total stories | {} | {total_points} |", rows.len()),
    );

    for status in SPRINT_STATUS_DISPLAY_ORDER {
        let items = rows_by_status.remove(status).unwrap_or_default();
        let points = items
            .iter()
            .map(|row| story_points_value(&row.story))
            .sum::<usize>();
        push_line(
            output,
            &format!(
                "| {} | {} | {points} |",
                status_summary_label(status),
                items.len()
            ),
        );
    }
}

fn sprint_story_link_label(row: &SprintRosterEntry) -> String {
    let title = row.story.title.trim();
    let label = if title.is_empty() {
        format!("**{}**", row.story.id)
    } else {
        format!("**{}** {}", row.story.id, title)
    };
    let link_text = escape_markdown_link_text(&label);
    format!("[{link_text}]({})", to_forward_slashes(&row.link_path))
}

fn render_assignee_cell(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" || trimmed.eq_ignore_ascii_case("tbd") {
        return "-".to_string();
    }

    let pattern =
        Regex::new(r"(?P<name>[^<]+?)\s*<(?P<email>[^>]+)>").expect("valid assignee parse regex");
    let assignees = parse_assignee_list(trimmed);
    let links = assignees
        .iter()
        .filter_map(|assignee| pattern.captures(assignee))
        .filter_map(|captures| {
            let name = captures.name("name")?.as_str().trim();
            let email = captures.name("email")?.as_str().trim();
            if name.is_empty() || email.is_empty() {
                return None;
            }
            Some(format!(
                "[{}](mailto:{})",
                escape_markdown_link_text(name),
                escape_markdown_link_target(email)
            ))
        })
        .collect::<Vec<_>>();

    if links.is_empty() {
        escape_table_cell(trimmed)
    } else {
        links.join(" and ")
    }
}

fn escape_markdown_link_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('|', "\\|")
}

fn escape_markdown_link_target(value: &str) -> String {
    value.replace(' ', "%20")
}

fn escape_table_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', " ")
        .trim()
        .to_string()
}

fn story_points_value(story: &StoryOverview) -> usize {
    story.story_points.trim().parse::<usize>().unwrap_or(0)
}

fn format_task_summary(summary: Option<&TaskSummary>) -> String {
    match summary {
        Some(summary) => format!(
            "✓{} ▶{} ·{} ✗{}",
            summary.done, summary.in_progress, summary.todo, summary.blocked
        ),
        None => "-".to_string(),
    }
}

fn status_summary_label(status: &str) -> &'static str {
    match status {
        "backlog" | "ready" => "Ready",
        "planned" => "Planned",
        "todo" => "Todo",
        "in-progress" => "In progress",
        "ready-for-qa" => "Ready for QA",
        "done" => "Done",
        "blocked" => "Blocked",
        _ => "Other",
    }
}

pub(crate) fn sprint_story_link_path(
    repo_root: &Path,
    sprint_file_path: &Path,
    story_relative_path: &Path,
) -> PathBuf {
    let sprint_dir = sprint_file_path.parent().unwrap_or(sprint_file_path);
    let story_path = repo_root.join(story_relative_path);
    relative_path_from(sprint_dir, &story_path)
}

fn relative_path_from(from: &Path, to: &Path) -> PathBuf {
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let shared_prefix = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();
    for _ in shared_prefix..from_components.len() {
        relative.push("..");
    }
    for component in to_components.iter().skip(shared_prefix) {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

pub(crate) fn replace_roster_in_body(body: &str, roster: &str) -> String {
    let trimmed = match body
        .find(ROSTER_HEADING)
        .or_else(|| body.find(LEGACY_ROSTER_HEADING))
    {
        Some(idx) => body[..idx].trim_end().to_string(),
        None => body.trim_end().to_string(),
    };
    format!("{trimmed}\n\n{roster}")
}

pub(crate) fn regenerate_sprint_roster(config: &KanbanConfig, sprint_name: &str) -> Result<bool> {
    let sprint_file = config.sprints_path().join(format!("{sprint_name}.md"));
    if !sprint_file.is_file() {
        return Ok(false);
    }
    let repository = read_repository(&config.repo_root)?;
    let rows = repository
        .stories
        .iter()
        .filter(|story| story.frontmatter.get("sprint").map(String::as_str) == Some(sprint_name))
        .map(|story| {
            let overview = story_overview(&repository.repo_root, story);
            let link_path = sprint_story_link_path(
                &repository.repo_root,
                &sprint_file,
                &overview.relative_path,
            );
            SprintRosterEntry {
                story: overview,
                link_path,
            }
        })
        .collect::<Vec<_>>();

    let content = fs::read_to_string(&sprint_file)
        .with_context(|| format!("read sprint file {}", sprint_file.display()))?;
    let parsed = parse_frontmatter(&content);
    let fm_block = frontmatter_region(&content)?;
    let new_body = replace_roster_in_body(&parsed.body, &render_sprint_roster(&rows));
    let updated = format!("{fm_block}{new_body}\n");
    if updated == content {
        return Ok(false);
    }
    atomic_write(&sprint_file, &updated)
        .with_context(|| format!("write sprint file {}", sprint_file.display()))?;
    Ok(true)
}

pub fn sync_sprint_rosters(repo_root: impl AsRef<Path>) -> Result<Vec<String>> {
    let config = load_kanban_config(repo_root)?;
    let mut changed = Vec::new();
    for name in list_sprint_names(&config.repo_root)? {
        if regenerate_sprint_roster(&config, &name)? {
            changed.push(name);
        }
    }
    Ok(changed)
}

pub(crate) fn update_sprint_summary_for_rollover(
    markdown: &str,
    sprint_name: &str,
    next_sprint_name: &str,
    completed_story_ids: &[String],
    carried_story_ids: &[String],
) -> String {
    let end_summary = if completed_story_ids.is_empty() {
        format!("Sprint closed. No stories were completed in `{sprint_name}` before rollover.")
    } else {
        format!(
            "Sprint closed. Completed stories in `{sprint_name}`: {}.",
            completed_story_ids.join(", ")
        )
    };
    let carry_over = if carried_story_ids.is_empty() {
        "No unfinished stories were moved forward.".to_string()
    } else {
        format!(
            "Moved to `{next_sprint_name}`: {}.",
            carried_story_ids.join(", ")
        )
    };
    let updated = replace_markdown_section(markdown, "End-Of-Sprint Summary", &end_summary);
    replace_markdown_section(
        &updated,
        "Expected Carry-Over / Unfinished Stories",
        &carry_over,
    )
}

pub(crate) fn sprint_warnings(
    repo_root: &Path,
    repository: &Repository,
    spec: &SprintFolderSpec,
    today: NaiveDate,
) -> Vec<String> {
    doctor_findings_for_sprint(repo_root, repository, spec, today)
        .into_iter()
        .map(|finding| finding.message)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use tempfile::tempdir;

    #[test]
    fn summarize_current_sprint_uses_sprint_file_dates() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2026-05-18",
            "2026-05-29",
            "planned",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md",
            "id: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let sprint = summarize_current_sprint_at_date(
            temp_root.path(),
            NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
        )
        .unwrap();

        assert_eq!(sprint.sprint_name, "S001.foundation");
        assert_eq!(
            sprint.sprint_goal.as_deref(),
            Some("Keep the team aligned on a visible sprint outcome.")
        );
        assert_eq!(sprint.readme_status.as_deref(), Some("planned"));
    }

    #[test]
    fn summarize_current_sprint_prefers_single_active_sprint_when_dates_are_overdue() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2026-05-18",
            "2026-05-29",
            "active",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md",
            "id: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let sprint = summarize_current_sprint_at_date(
            temp_root.path(),
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
        )
        .unwrap();

        assert_eq!(sprint.sprint_name, "S001.foundation");
        assert_eq!(sprint.readme_status.as_deref(), Some("active"));
    }

    #[test]
    fn list_current_sprint_stories_returns_flattened_current_sprint_rows() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            "2026-05-18",
            "2026-05-29",
            "active",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md",
            "id: US-F1-052\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md",
            "id: US-F1-053\ntype: user-story\nstatus: in-progress\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 8\nwork_started: 2026-05-28T14:05:54+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let (sprint_name, stories) = list_current_sprint_stories(temp_root.path()).unwrap();

        assert_eq!(sprint_name, "S001.foundation");
        assert_eq!(stories.len(), 2);
        assert_eq!(stories[0].id, "US-F1-052");
        assert_eq!(stories[1].id, "US-F1-053");
    }

    #[test]
    fn list_next_sprint_stories_uses_next_numbered_sprint_after_current() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let today = Local::now().date_naive();
        let current_start = today.checked_sub_days(Days::new(1)).unwrap().to_string();
        let current_end = today.checked_add_days(Days::new(1)).unwrap().to_string();
        let next_start = today.checked_add_days(Days::new(2)).unwrap().to_string();
        let next_end = today.checked_add_days(Days::new(13)).unwrap().to_string();

        write_sprint_file(
            temp_root.path(),
            "S001.foundation",
            "foundation",
            &current_start,
            &current_end,
            "active",
        );
        write_sprint_file(
            temp_root.path(),
            "S002.delivery",
            "delivery",
            &next_start,
            &next_end,
            "planned",
        );
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-054-add-cli-support-for-completing-tasks-from-the-terminal.md",
            "id: US-F1-054\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S002.delivery\nassignee: TBD\nstory_points: 3\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let (sprint_name, stories) = list_next_sprint_stories(temp_root.path()).unwrap();

        assert_eq!(sprint_name, "S002.delivery");
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].id, "US-F1-054");
    }

    #[test]
    fn create_sprint_creates_single_file_and_readme() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        fs::create_dir_all(temp_root.path().join("delivery/sprints")).unwrap();
        let today = Local::now().date_naive();
        let input = CreateSprintInput {
            number: 1,
            start_date: today,
            end_date: today + Days::new(11),
            headline: "Foundation Sprint".to_string(),
        };

        let result = create_sprint(temp_root.path(), &input).unwrap();

        assert_eq!(result.sprint_name, "S001.foundation-sprint");
        let sprint_file = temp_root.path().join(&result.sprint_path);
        assert!(sprint_file.exists());
        let markdown = fs::read_to_string(sprint_file).unwrap();
        assert!(markdown.contains("status: planned"));
        assert!(markdown.contains(ROSTER_HEADING));
        assert!(markdown.contains("| Metric | Stories | Points |"));
        assert!(markdown.contains("| Total stories | 0 | 0 |"));
        assert!(markdown.contains("| Story | Points | Assignee | Tasks |"));
        assert!(markdown.contains("| — | — | — | — |"));
    }

    #[test]
    fn render_assignee_cell_links_comma_separated_assignees_without_leading_comma() {
        let rendered = render_assignee_cell(
            "Thomas Malt <thomas.malt@vegvesen.no>, Sondre Bjerkerud <sondre.bjerkerud@soprasteria.com>",
        );

        assert_eq!(
            rendered,
            "[Thomas Malt](mailto:thomas.malt@vegvesen.no) and [Sondre Bjerkerud](mailto:sondre.bjerkerud@soprasteria.com)"
        );
    }

    #[test]
    fn regenerate_sprint_roster_rewrites_legacy_heading_with_story_links() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let sprint_file = temp_root.path().join("delivery/sprints/S001.foundation.md");
        fs::create_dir_all(sprint_file.parent().unwrap()).unwrap();
        fs::write(
            &sprint_file,
            "---\nsprint: S001\nheadline: foundation\nstart_date: 2026-05-18\nend_date: 2026-05-29\nstatus: active\nwip_limit: null\n---\n\n# S001: foundation\n\n## Sprint Goal\n\nKeep the team aligned on a visible sprint outcome.\n\n## Notes For Review / Demo\n\n- Sprint created by `kanban sprint create`.\n\n## End-Of-Sprint Summary\n\nSprint not started yet.\n\n## Expected Carry-Over / Unfinished Stories\n\nNot determined yet.\n\n## Stories (generated — do not edit)\n\n| Metric | Stories | Points |\n|--------|--------:|------:|\n| Total stories | 1 | 5 |\n| Todo | 1 | 5 |\n| In progress | 0 | 0 |\n| Ready for QA | 0 | 0 |\n| Done | 0 | 0 |\n| Blocked | 0 | 0 |\n\n### todo\n\n| Story | Points | Assignee | Tasks |\n|-------|-------:|----------|-------|\n| [**US-F1-001** Test story](../backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-001-kubernetes-cluster-for-development-environment.md) | 5 | [Test User](mailto:test@example.com) | - |\n",
        )
        .unwrap();
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-001-kubernetes-cluster-for-development-environment.md",
            "id: US-F1-001\ntype: user-story\nstatus: todo\nepic: EP-F1-01\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let config = load_kanban_config(temp_root.path()).unwrap();
        let changed = regenerate_sprint_roster(&config, "S001.foundation").unwrap();
        let markdown = fs::read_to_string(&sprint_file).unwrap();

        assert!(changed);
        assert!(markdown.contains(ROSTER_HEADING));
        assert!(!markdown.contains("## Stories (generated — do not edit)"));
        assert!(markdown.contains("| Metric | Stories | Points |"));
        assert!(markdown.contains("### todo"));
        assert!(markdown.contains("| Story | Points | Assignee | Tasks |"));
        assert!(markdown.contains(
            "[**US-F1-001** Test story](../backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-001-kubernetes-cluster-for-development-environment.md)"
        ));
        assert!(markdown.contains("mailto:test@example.com"));
    }

    #[test]
    fn regenerate_sprint_roster_preserves_crlf_frontmatter_closing_delimiter() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let sprint_file = temp_root.path().join("delivery/sprints/S001.foundation.md");
        fs::create_dir_all(sprint_file.parent().unwrap()).unwrap();
        fs::write(
            &sprint_file,
            "---\r\nsprint: S001\r\nheadline: foundation\r\nstart_date: 2026-05-18\r\nend_date: 2026-05-29\r\nstatus: active\r\nwip_limit: null\r\n---\r\n\r\n# S001: foundation\r\n\r\n## Sprint Goal\r\n\r\nKeep the team aligned.\r\n\r\n## Stories (generated — do not edit)\r\n\r\nOld roster.\r\n",
        )
        .unwrap();
        write_story(
            temp_root.path(),
            "doc/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling/US-F1-001-kubernetes-cluster-for-development-environment.md",
            "id: US-F1-001\ntype: user-story\nstatus: todo\nepic: EP-F1-01\nsprint: S001.foundation\nassignee: Test User <test@example.com>\nstory_points: 5\nwork_started:\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T14:05:54+0200\n",
        );

        let config = load_kanban_config(temp_root.path()).unwrap();
        let changed = regenerate_sprint_roster(&config, "S001.foundation").unwrap();
        let markdown = fs::read_to_string(&sprint_file).unwrap();

        assert!(changed);
        assert!(markdown.starts_with("---\r\nsprint: S001\r\n"));
        assert!(markdown.contains("wip_limit: null\r\n---\r\n"));
        assert!(markdown.contains("# S001: foundation"));
        assert!(!markdown.contains("wip_limit: nul\r\n# S001"));
        assert!(markdown.contains(ROSTER_HEADING));
    }

    #[test]
    fn create_sprint_uses_configured_sprints_path() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        set_config_value(temp_root.path(), "paths.sprints", "planning/sprints").unwrap();
        let today = Local::now().date_naive();
        let input = CreateSprintInput {
            number: 1,
            start_date: today,
            end_date: today + Days::new(11),
            headline: "Foundation Sprint".to_string(),
        };

        let result = create_sprint(temp_root.path(), &input).unwrap();

        assert_eq!(
            result.sprint_path,
            PathBuf::from("planning/sprints/S001.foundation-sprint.md")
        );
        assert!(
            temp_root
                .path()
                .join("planning/sprints/S001.foundation-sprint.md")
                .exists()
        );
    }

    #[test]
    fn suggested_next_sprint_dates_use_latest_sprint_file_end_date() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let sprints_root = temp_root.path().join("delivery/sprints");
        fs::create_dir_all(&sprints_root).unwrap();
        fs::write(
            sprints_root.join("S000.getting-started.md"),
            sprint_readme(
                "S000",
                "getting-started",
                "2026-05-18",
                "2026-05-29",
                "closed",
            ),
        )
        .unwrap();
        fs::write(
            sprints_root.join("S001.foundation.md"),
            sprint_readme("S001", "foundation", "2026-06-02", "2026-06-13", "planned"),
        )
        .unwrap();

        let suggestion = suggested_next_sprint_dates(temp_root.path())
            .unwrap()
            .unwrap();

        assert_eq!(suggestion.0, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
        assert_eq!(suggestion.1, NaiveDate::from_ymd_opt(2026, 6, 26).unwrap());
    }

    #[test]
    fn rollover_moves_unfinished_stories_and_updates_closed_summary() {
        let temp_root = tempdir().unwrap();
        init_temp_repo(temp_root.path());
        let backlog_dir = temp_root
            .path()
            .join("delivery/backlog/phase-1-scaffolding/06.git-driven-kanban-and-backlog-tooling");

        fs::create_dir_all(&backlog_dir).unwrap();
        let sprint_file = temp_root.path().join("delivery/sprints/S001.foundation.md");
        fs::create_dir_all(sprint_file.parent().unwrap()).unwrap();
        fs::write(
            &sprint_file,
            format!(
                "{}\n## End-Of-Sprint Summary\n\nSprint still active.\n\n## Expected Carry-Over / Unfinished Stories\n\nNot determined yet.\n",
                sprint_readme("S001", "foundation", "2099-06-01", "2099-06-12", "active")
            ),
        ).unwrap();
        fs::write(
            backlog_dir.join("US-F1-052-add-read-only-cli-for-sprint-and-backlog-inspection.md"),
            "---\nid: US-F1-052\ntype: user-story\nstatus: done\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 5\nwork_started: 2026-05-28T16:30:54+0200\nwork_done: 2026-05-28T22:06:38+0200\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T22:06:38+0200\n---\n# User Story: Add read-only CLI for sprint and backlog inspection\n",
        ).unwrap();
        fs::write(
            backlog_dir.join("US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md"),
            "---\nid: US-F1-053\ntype: user-story\nstatus: todo\nepic: EP-F1-06\nsprint: S001.foundation\nassignee: TBD\nstory_points: 8\nwork_started: 2026-05-28T22:35:00+0200\nwork_done:\ncreated: 2026-05-28T14:05:54+0200\nupdated: 2026-05-28T22:35:00+0200\n---\n# User Story: Add CLI support for status moves and sprint rollover\n",
        ).unwrap();

        let next_start = NaiveDate::from_ymd_opt(2099, 6, 15).unwrap();
        let next_end = NaiveDate::from_ymd_opt(2099, 6, 26).unwrap();
        let next_input = CreateSprintInput {
            number: 2,
            start_date: next_start,
            end_date: next_end,
            headline: "next-sprint".to_string(),
        };

        let result =
            rollover_sprint(temp_root.path(), "S001.foundation", Some(&next_input)).unwrap();

        assert!(result.created_next_sprint);
        assert_eq!(result.completed_story_ids, vec!["US-F1-052".to_string()]);
        assert_eq!(result.carried_story_ids, vec!["US-F1-053".to_string()]);
        let carried_story = fs::read_to_string(
            backlog_dir.join("US-F1-053-add-cli-support-for-status-moves-and-sprint-rollover.md"),
        )
        .unwrap();
        assert!(carried_story.contains("sprint: S002.next-sprint"));
        let closed_summary = fs::read_to_string(&sprint_file).unwrap();
        assert!(closed_summary.contains("Completed stories in `S001.foundation`: US-F1-052."));
        assert!(closed_summary.contains("Moved to `S002.next-sprint`: US-F1-053."));
    }
}
