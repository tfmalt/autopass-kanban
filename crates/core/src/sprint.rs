use crate::config::*;
use crate::constants::*;
use crate::doctor::*;
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
        .ok_or_else(|| anyhow!("Sprint not found: {sprint_name}"))
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
    fs::write(&sprint_file, content)
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
        .ok_or_else(|| anyhow!("Sprint not found: {sprint_name}"))?;

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
        fs::write(&story.file_path, moved_story_markdown).with_context(|| {
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
    fs::write(&closed_readme_path, closed_readme)
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
        stories_by_status
            .entry(overview.status.clone())
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

pub(crate) fn render_sprint_roster(rows: &[(String, String)]) -> String {
    let mut out = format!("{ROSTER_HEADING}\n\n");
    for column in ROSTER_COLUMN_ORDER {
        let mut ids: Vec<&str> = rows
            .iter()
            .filter(|(_, status)| status == column)
            .map(|(id, _)| id.as_str())
            .collect();
        ids.sort_unstable();
        if ids.is_empty() {
            out.push_str(&format!("- {column}: —\n"));
        } else {
            out.push_str(&format!("- {column}: {}\n", ids.join(", ")));
        }
    }
    out
}

pub(crate) fn replace_roster_in_body(body: &str, roster: &str) -> String {
    let trimmed = match body.find(ROSTER_HEADING) {
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
    let mut rows = repository
        .stories
        .iter()
        .filter(|story| story.frontmatter.get("sprint").map(String::as_str) == Some(sprint_name))
        .filter_map(|story| {
            Some((
                story.frontmatter.get("id")?.clone(),
                story.frontmatter.get("status").cloned().unwrap_or_default(),
            ))
        })
        .collect::<Vec<_>>();
    rows.sort();

    let content = fs::read_to_string(&sprint_file)
        .with_context(|| format!("read sprint file {}", sprint_file.display()))?;
    let parsed = parse_frontmatter(&content);
    let fm_block = frontmatter_region(&content)?;
    let new_body = replace_roster_in_body(&parsed.body, &render_sprint_roster(&rows));
    let updated = format!("{fm_block}{new_body}\n");
    if updated == content {
        return Ok(false);
    }
    fs::write(&sprint_file, updated)
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
