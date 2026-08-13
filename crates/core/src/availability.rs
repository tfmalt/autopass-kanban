use crate::config::{KanbanConfig, TeamMemberConfig};
use crate::prelude::*;
use crate::util::ensure_path_inside;

const TABLE_HEADER: [&str; 7] = ["id", "type", "who", "start", "end", "availability", "note"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityKind {
    Holiday,
    Hiatus,
    Vacation,
    Absence,
}

impl AvailabilityKind {
    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "holiday" => Ok(Self::Holiday),
            "hiatus" => Ok(Self::Hiatus),
            "vacation" => Ok(Self::Vacation),
            "absence" => Ok(Self::Absence),
            _ => bail!("availability type must be one of holiday, hiatus, vacation, or absence"),
        }
    }

    fn is_team_wide(self) -> bool {
        matches!(self, Self::Holiday | Self::Hiatus)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailabilityEntry {
    pub id: String,
    pub kind: AvailabilityKind,
    /// `None` represents the whole team (`*`). Individual entries store the
    /// canonical email from `.kanban/settings.json`.
    pub who: Option<String>,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub availability_percent: u8,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkingCalendar {
    entries: Vec<AvailabilityEntry>,
    team: Vec<String>,
}

impl WorkingCalendar {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn load(config: &KanbanConfig) -> Result<Self> {
        let configured_path = config.availability_path();
        if !configured_path.exists() {
            return Ok(Self::empty());
        }
        let path = ensure_path_inside(&config.repo_root, &configured_path)?;
        let markdown = fs::read_to_string(&path)
            .with_context(|| format!("read availability calendar {}", path.display()))?;
        parse_availability_markdown(&markdown, &config.team)
            .with_context(|| format!("parse availability calendar {}", path.display()))
    }

    pub fn entries(&self) -> &[AvailabilityEntry] {
        &self.entries
    }

    pub fn day_capacity(&self, date: NaiveDate) -> f64 {
        if date.weekday().number_from_monday() > 5 {
            return 0.0;
        }

        let matching = self
            .entries
            .iter()
            .filter(|entry| entry.start <= date && date <= entry.end)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return 1.0;
        }

        let team_wide = matching
            .iter()
            .filter(|entry| entry.who.is_none())
            .map(|entry| entry.availability_percent)
            .min()
            .unwrap_or(100);
        if self.team.is_empty() {
            return f64::from(team_wide) / 100.0;
        }

        let total = self
            .team
            .iter()
            .map(|member| {
                matching
                    .iter()
                    .filter(|entry| {
                        entry.who.is_none() || entry.who.as_deref() == Some(member.as_str())
                    })
                    .map(|entry| entry.availability_percent)
                    .min()
                    .unwrap_or(100)
            })
            .map(u32::from)
            .sum::<u32>();
        total as f64 / (self.team.len() as f64 * 100.0)
    }

    pub fn is_working_day(&self, date: NaiveDate) -> bool {
        self.day_capacity(date) > 0.0
    }

    pub fn capacity_sum(&self, start: NaiveDate, end: NaiveDate) -> f64 {
        if end < start {
            return 0.0;
        }
        let mut date = start;
        let mut capacity = 0.0;
        while date <= end {
            capacity += self.day_capacity(date);
            date += chrono::Duration::days(1);
        }
        capacity
    }

    pub fn add_capacity_days(&self, start: NaiveDate, days: f64) -> NaiveDate {
        if days <= 0.0 {
            return start;
        }
        let mut remaining = days;
        let mut date = start;
        while remaining > 0.0 {
            let Some(next) = date.checked_add_signed(chrono::Duration::days(1)) else {
                return date;
            };
            date = next;
            remaining -= self.day_capacity(date);
        }
        date
    }
}

pub fn parse_availability_markdown(
    markdown: &str,
    team: &[TeamMemberConfig],
) -> Result<WorkingCalendar> {
    let mut lines = markdown.lines().enumerate();
    let Some((_, _)) = lines.find(|(_, line)| {
        parse_table_row(line).is_some_and(|cells| {
            cells
                .iter()
                .map(|cell| cell.to_ascii_lowercase())
                .eq(TABLE_HEADER.iter().map(|value| (*value).to_string()))
        })
    }) else {
        bail!(
            "availability calendar must contain a table with columns: {}",
            TABLE_HEADER.join(", ")
        );
    };

    let Some((_, separator)) = lines.next() else {
        bail!("availability table is missing its separator row");
    };
    if parse_table_row(separator).is_none_or(|cells| {
        cells.len() != TABLE_HEADER.len()
            || cells
                .iter()
                .any(|cell| !cell.trim_matches([':', '-', ' ']).is_empty())
    }) {
        bail!("availability table has an invalid separator row");
    }

    let mut entries = Vec::new();
    let mut ids = BTreeSet::new();
    let mut table_ended = false;
    for (line_index, line) in lines {
        if line.trim().is_empty() {
            table_ended = true;
            continue;
        }
        let Some(cells) = parse_table_row(line) else {
            table_ended = true;
            continue;
        };
        if table_ended {
            bail!(
                "availability row {} appears after the table has ended",
                line_index + 1
            );
        }
        if cells.len() != TABLE_HEADER.len() {
            bail!(
                "availability row {} must contain {} columns",
                line_index + 1,
                TABLE_HEADER.len()
            );
        }
        let entry = parse_entry(&cells, team)
            .with_context(|| format!("invalid availability row {}", line_index + 1))?;
        if !ids.insert(entry.id.to_ascii_lowercase()) {
            bail!("duplicate availability ID {}", entry.id);
        }
        entries.push(entry);
    }

    Ok(WorkingCalendar {
        entries,
        team: team.iter().map(|member| member.email.clone()).collect(),
    })
}

fn parse_entry(cells: &[String], team: &[TeamMemberConfig]) -> Result<AvailabilityEntry> {
    let id = cells[0].trim().to_string();
    if id.is_empty() {
        bail!("availability ID must not be empty");
    }
    let kind = AvailabilityKind::parse(&cells[1])?;
    let who_value = cells[2].trim();
    let who = if who_value == "*" {
        None
    } else {
        let member = team.iter().find(|member| {
            member.email.eq_ignore_ascii_case(who_value)
                || member.name.eq_ignore_ascii_case(who_value)
        });
        Some(
            member
                .map(|member| member.email.clone())
                .ok_or_else(|| anyhow!("unknown team member {who_value:?}"))?,
        )
    };
    if kind.is_team_wide() && who.is_some() {
        bail!("holiday and hiatus entries must use `*` in the Who column");
    }
    if !kind.is_team_wide() && who.is_none() {
        bail!("vacation and absence entries must identify a team member");
    }

    let start = NaiveDate::parse_from_str(cells[3].trim(), "%Y-%m-%d")
        .context("start must use YYYY-MM-DD")?;
    let end = NaiveDate::parse_from_str(cells[4].trim(), "%Y-%m-%d")
        .context("end must use YYYY-MM-DD")?;
    if end < start {
        bail!("end must be on or after start");
    }
    let availability_percent = cells[5]
        .trim()
        .trim_end_matches('%')
        .parse::<u8>()
        .context("availability must be a percentage from 0% to 100%")?;
    if availability_percent > 100 {
        bail!("availability must be a percentage from 0% to 100%");
    }

    Ok(AvailabilityEntry {
        id,
        kind,
        who,
        start,
        end,
        availability_percent,
        note: cells[6].trim().to_string(),
    })
}

fn parse_table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return None;
    }
    let content = trimmed
        .strip_prefix('|')?
        .strip_suffix('|')
        .unwrap_or_else(|| trimmed.strip_prefix('|').unwrap_or_default());
    Some(
        content
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team() -> Vec<TeamMemberConfig> {
        vec![
            TeamMemberConfig {
                name: "Ada Lovelace".to_string(),
                email: "ada@example.com".to_string(),
                avatar_url: None,
                avatar_path: None,
            },
            TeamMemberConfig {
                name: "Grace Hopper".to_string(),
                email: "grace@example.com".to_string(),
                avatar_url: None,
                avatar_path: None,
            },
        ]
    }

    fn calendar(rows: &str) -> WorkingCalendar {
        parse_availability_markdown(
            &format!(
                "# Availability\n\n| ID | Type | Who | Start | End | Availability | Note |\n|---|---|---|---|---|---:|---|\n{rows}"
            ),
            &team(),
        )
        .unwrap()
    }

    #[test]
    fn team_hiatus_removes_capacity_for_the_date_range() {
        let calendar =
            calendar("| AV-001 | hiatus | * | 2026-07-01 | 2026-07-28 | 0% | Summer shutdown |");

        assert_eq!(
            calendar.day_capacity(NaiveDate::from_ymd_opt(2026, 7, 15).unwrap()),
            0.0
        );
        assert_eq!(
            calendar.day_capacity(NaiveDate::from_ymd_opt(2026, 7, 29).unwrap()),
            1.0
        );
    }

    #[test]
    fn individual_and_overlapping_entries_use_most_restrictive_capacity() {
        let calendar = calendar(
            "| AV-001 | vacation | ada@example.com | 2026-08-03 | 2026-08-03 | 0% | Vacation |\n| AV-002 | absence | Ada Lovelace | 2026-08-03 | 2026-08-03 | 50% | Course |",
        );

        assert_eq!(
            calendar.day_capacity(NaiveDate::from_ymd_opt(2026, 8, 3).unwrap()),
            0.5
        );
    }

    #[test]
    fn unknown_member_is_rejected() {
        let error = parse_availability_markdown(
            "| ID | Type | Who | Start | End | Availability | Note |\n|---|---|---|---|---|---:|---|\n| AV-001 | vacation | unknown@example.com | 2026-08-03 | 2026-08-03 | 0% | Vacation |",
            &team(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("invalid availability row"));
        assert!(format!("{error:#}").contains("unknown team member"));
    }

    #[test]
    fn rows_after_a_blank_line_are_rejected_instead_of_ignored() {
        let error = parse_availability_markdown(
            "| ID | Type | Who | Start | End | Availability | Note |\n|---|---|---|---|---|---:|---|\n| AV-001 | hiatus | * | 2026-07-01 | 2026-07-02 | 0% | Pause |\n\n| AV-002 | hiatus | * | 2026-07-03 | 2026-07-04 | 0% | More pause |",
            &team(),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("appears after the table has ended")
        );
    }

    #[test]
    fn rows_may_omit_the_optional_closing_pipe() {
        let calendar = parse_availability_markdown(
            "| ID | Type | Who | Start | End | Availability | Note\n|---|---|---|---|---|---:|---\n| AV-001 | hiatus | * | 2026-07-01 | 2026-07-02 | 0% | Pause",
            &team(),
        )
        .unwrap();

        assert_eq!(calendar.entries().len(), 1);
    }

    #[test]
    fn capacity_days_skip_hiatus_and_apply_partial_days() {
        let calendar = calendar(
            "| AV-001 | hiatus | * | 2026-08-04 | 2026-08-04 | 0% | Pause |\n| AV-002 | absence | ada@example.com | 2026-08-05 | 2026-08-05 | 0% | Away |",
        );
        let start = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();

        assert_eq!(
            calendar.add_capacity_days(start, 1.5),
            NaiveDate::from_ymd_opt(2026, 8, 6).unwrap()
        );
    }
}
