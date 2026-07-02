use serde::Serialize;

use crate::{SprintOverview, StoryOverview, StoryStatus};

use super::{ForecastInputs, ReportForecastDto, parse_points, path_string};

fn phase_from_story_id(id: &str) -> String {
    id.split('-').nth(1).unwrap_or("unknown").to_string()
}

/// Per-story row in the WBS report.
#[derive(Debug, Clone, Serialize)]
pub struct ReportStoryDto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub story_points: Option<i64>,
    pub sprint: Option<String>,
    pub epic_id: Option<String>,
    pub epic_title: Option<String>,
    pub phase: Option<String>,
    pub path: String,
    pub work_started: Option<String>,
    pub work_done: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
}

impl ReportStoryDto {
    pub fn from_overview(o: &StoryOverview) -> Self {
        Self {
            phase: Some(phase_from_story_id(&o.id)),
            id: o.id.clone(),
            title: o.title.clone(),
            status: o.status.clone(),
            story_points: parse_points(&o.story_points),
            sprint: o.sprint.clone(),
            epic_id: o.epic_id.clone(),
            epic_title: o.epic_title.clone(),
            path: path_string(&o.relative_path),
            work_started: o.work_started.clone(),
            work_done: o.work_done.clone(),
            planned_start: o.planned_start.clone(),
            planned_end: o.planned_end.clone(),
        }
    }
}

/// Per-sprint burndown row in the WBS report.
#[derive(Debug, Clone, Serialize)]
pub struct ReportSprintDto {
    pub sprint_name: String,
    pub start_date: String,
    pub end_date: String,
    pub is_current: bool,
    pub is_past: bool,
    pub planned_points: i64,
    pub delivered_points: i64,
    pub story_ids: Vec<String>,
}

/// Per-phase summary row.
#[derive(Debug, Clone, Serialize)]
pub struct ReportPhaseDto {
    pub phase: String,
    pub story_count: usize,
    pub points_total: i64,
    pub points_done: i64,
    pub points_in_progress: i64,
    pub points_remaining: i64,
}

/// Velocity and prognosis summary.
#[derive(Debug, Clone, Serialize)]
pub struct ReportVelocityDto {
    pub completed_sprint_count: usize,
    pub avg_points_per_sprint: f64,
    pub remaining_points: i64,
    pub estimated_sprints_remaining: Option<f64>,
    pub sprint_duration_weeks: u32,
}

/// Top-level payload for `kanban report wbs --format json`.
#[derive(Debug, Clone, Serialize)]
pub struct ReportWbsDto {
    pub generated_at: String,
    pub stories: Vec<ReportStoryDto>,
    pub sprints: Vec<ReportSprintDto>,
    pub phases: Vec<ReportPhaseDto>,
    pub velocity: ReportVelocityDto,
    pub forecast: ReportForecastDto,
}

fn average(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<i64>() as f64 / values.len() as f64
    }
}

struct PreparedReport {
    stories: Vec<ReportStoryDto>,
    sprints: Vec<ReportSprintDto>,
    phases: Vec<ReportPhaseDto>,
    velocity: ReportVelocityDto,
    forecast_inputs: ForecastInputs,
}

impl PreparedReport {
    fn build(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        current_sprint_name: Option<&str>,
        generated_at: String,
        today: chrono::NaiveDate,
    ) -> Self {
        let mut sprint_stats: std::collections::BTreeMap<String, (i64, i64, Vec<String>)> =
            std::collections::BTreeMap::new();
        for story in stories {
            if let Some(ref sprint) = story.sprint {
                let pts = parse_points(&story.story_points).unwrap_or(0);
                let entry = sprint_stats.entry(sprint.clone()).or_default();
                entry.0 += pts;
                if StoryStatus::parse(&story.status) == Some(StoryStatus::Done) {
                    entry.1 += pts;
                }
                entry.2.push(story.id.clone());
            }
        }

        let sprint_dtos: Vec<ReportSprintDto> = sprints
            .iter()
            .map(|s| {
                let end =
                    chrono::NaiveDate::parse_from_str(&s.end_date, "%Y-%m-%d").unwrap_or(today);
                let is_past = end < today;
                let is_current = Some(s.sprint_name.as_str()) == current_sprint_name;
                let (planned, done, ids) = sprint_stats
                    .get(&s.sprint_name)
                    .cloned()
                    .unwrap_or_default();
                ReportSprintDto {
                    sprint_name: s.sprint_name.clone(),
                    start_date: s.start_date.clone(),
                    end_date: s.end_date.clone(),
                    is_current,
                    is_past,
                    planned_points: planned,
                    delivered_points: done,
                    story_ids: ids,
                }
            })
            .collect();

        let mut phase_map: std::collections::BTreeMap<String, (usize, i64, i64, i64, i64)> =
            std::collections::BTreeMap::new();
        for story in stories {
            let phase = phase_from_story_id(&story.id);
            let pts = parse_points(&story.story_points).unwrap_or(0);
            let e = phase_map.entry(phase).or_default();
            e.0 += 1;
            e.1 += pts;
            let status = story.status.to_ascii_lowercase();
            if status == "done" {
                e.2 += pts;
            } else if status == "in-progress" || status == "ready-for-qa" {
                e.3 += pts;
            } else {
                e.4 += pts;
            }
        }
        let phase_dtos: Vec<ReportPhaseDto> = phase_map
            .into_iter()
            .map(|(phase, (count, total, done, wip, rem))| ReportPhaseDto {
                phase,
                story_count: count,
                points_total: total,
                points_done: done,
                points_in_progress: wip,
                points_remaining: rem,
            })
            .collect();

        let past_with_stories: Vec<&ReportSprintDto> = sprint_dtos
            .iter()
            .filter(|s| s.is_past && s.planned_points > 0)
            .collect();
        let velocity_samples: Vec<i64> = past_with_stories
            .iter()
            .map(|s| s.delivered_points)
            .collect();
        let completed_count = velocity_samples.len();
        let avg_velocity = average(&velocity_samples);

        let remaining: i64 = stories
            .iter()
            .filter(|s| {
                StoryStatus::parse_counts_toward_scope(&s.status)
                    && !StoryStatus::parse_is_terminal(&s.status)
            })
            .map(|s| parse_points(&s.story_points).unwrap_or(0))
            .sum();

        let est_sprints = if avg_velocity > 0.0 {
            Some(remaining as f64 / avg_velocity)
        } else {
            None
        };

        let sprint_duration_weeks = sprint_dtos
            .first()
            .and_then(|s| {
                let start = chrono::NaiveDate::parse_from_str(&s.start_date, "%Y-%m-%d").ok()?;
                let end = chrono::NaiveDate::parse_from_str(&s.end_date, "%Y-%m-%d").ok()?;
                Some(((end - start).num_days() as f64 / 7.0).round() as u32)
            })
            .unwrap_or(2)
            .max(1);

        let velocity = ReportVelocityDto {
            completed_sprint_count: completed_count,
            avg_points_per_sprint: avg_velocity,
            remaining_points: remaining,
            estimated_sprints_remaining: est_sprints,
            sprint_duration_weeks,
        };
        let forecast_inputs = ForecastInputs {
            generated_at,
            remaining_points: remaining,
            sprint_duration_weeks,
            projection_start_date: today,
            throughput_samples: super::daily_throughput_samples(stories, today),
        };

        Self {
            stories: stories.iter().map(ReportStoryDto::from_overview).collect(),
            sprints: sprint_dtos,
            phases: phase_dtos,
            velocity,
            forecast_inputs,
        }
    }
}

impl ReportWbsDto {
    pub fn build(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        current_sprint_name: Option<&str>,
    ) -> Self {
        use chrono::Local;

        let today = Local::now().date_naive();
        let generated_at = Local::now().to_rfc3339();
        let prepared = PreparedReport::build(
            stories,
            sprints,
            current_sprint_name,
            generated_at.clone(),
            today,
        );
        let forecast = ReportForecastDto::from_inputs(prepared.forecast_inputs);

        ReportWbsDto {
            generated_at,
            stories: prepared.stories,
            sprints: prepared.sprints,
            phases: prepared.phases,
            velocity: prepared.velocity,
            forecast,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn report_story_dto_serializes_planned_dates_from_frontmatter_metadata() {
        let overview = crate::StoryOverview {
            id: "US-F1-058".to_string(),
            title: "Add planned and actual dates".to_string(),
            status: "todo".to_string(),
            epic_id: Some("EP-F1-06".to_string()),
            epic_title: Some("Git-driven kanban and backlog tooling".to_string()),
            assignee: "TBD".to_string(),
            story_points: "1".to_string(),
            sprint: Some("S001.scaffolding-part-1".to_string()),
            relative_path: PathBuf::from("delivery/backlog/x/US-F1-058.md"),
            task_summary: None,
            task_count: 0,
            work_started: Some("2026-06-11T10:00:00+0200".to_string()),
            work_done: None,
            planned_start: Some("2026-06-15".to_string()),
            planned_end: Some("2026-06-19".to_string()),
        };

        let dto = ReportStoryDto::from_overview(&overview);
        let json = serde_json::to_value(&dto).expect("serialization should succeed");

        assert_eq!(json["planned_start"], "2026-06-15");
        assert_eq!(json["planned_end"], "2026-06-19");
        assert_eq!(json["work_started"], "2026-06-11T10:00:00+0200");
        assert!(json["work_done"].is_null());
    }
}
