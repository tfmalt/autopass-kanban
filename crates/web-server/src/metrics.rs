use chrono::{Days, Local, NaiveDate};
use kanban_core::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use ts_rs::TS;

use crate::dto::{ProjectProgress, RepositorySnapshot, WebSprint, WebStory};
use crate::snapshot::compute_progress;

fn story_status(story: &WebStory) -> Option<StoryStatus> {
    StoryStatus::parse(&story.status)
}

fn story_counts_toward_scope(story: &WebStory) -> bool {
    StoryStatus::parse(&story.status).is_none_or(StoryStatus::counts_toward_scope)
}

fn story_is_done(story: &WebStory) -> bool {
    story_status(story) == Some(StoryStatus::Done)
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardMetrics {
    pub(crate) burndown: Vec<BurndownPoint>,
    pub(crate) burnup: Vec<BurnupPoint>,
    pub(crate) lead_time: Vec<LeadTimePoint>,
    pub(crate) velocity: Vec<VelocityPoint>,
    pub(crate) forecast: Forecast,
    pub(crate) progress: ProjectProgress,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
pub(crate) struct BurndownPoint {
    pub(crate) date: String,
    pub(crate) remaining: i64,
    pub(crate) ideal: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
pub(crate) struct BurnupPoint {
    pub(crate) date: String,
    pub(crate) completed: i64,
    pub(crate) scope: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LeadTimePoint {
    pub(crate) story_id: String,
    pub(crate) date: String,
    pub(crate) days: i64,
    pub(crate) rolling_avg: f64,
}

#[derive(Debug, Serialize, TS)]
pub(crate) struct VelocityPoint {
    pub(crate) sprint: String,
    pub(crate) points: i64,
    pub(crate) forecast: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Forecast {
    pub(crate) generated_at: String,
    pub(crate) remaining_points: i64,
    pub(crate) sprint_duration_weeks: i64,
    pub(crate) projection_start_date: String,
    pub(crate) throughput: ForecastThroughput,
    pub(crate) completion: ForecastCompletion,
    pub(crate) confidence: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForecastThroughput {
    pub(crate) samples: Vec<i64>,
    pub(crate) average: f64,
    pub(crate) median: f64,
    pub(crate) observed_day_count: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForecastCompletion {
    pub(crate) p50_days: Option<i64>,
    pub(crate) p80_days: Option<i64>,
    pub(crate) p90_days: Option<i64>,
    pub(crate) p50_date: Option<String>,
    pub(crate) p80_date: Option<String>,
    pub(crate) p90_date: Option<String>,
}

pub(crate) fn compute_metrics(
    repo: &RepositorySnapshot,
    core_stories: &[StoryOverview],
    core_sprints: &[SprintOverview],
) -> DashboardMetrics {
    let progress = compute_progress(&repo.stories);
    DashboardMetrics {
        burndown: build_burndown(&repo.sprints),
        burnup: build_burnup(&repo.stories, &repo.sprints),
        lead_time: build_lead_time(&repo.stories),
        velocity: build_velocity(&repo.sprints),
        forecast: build_forecast(core_stories, core_sprints),
        progress,
    }
}

pub(crate) fn build_burnup(stories: &[WebStory], sprints: &[WebSprint]) -> Vec<BurnupPoint> {
    let today = Local::now().date_naive();
    let mut completed_by_date = BTreeMap::<NaiveDate, i64>::new();
    for story in stories.iter().filter(|story| story_is_done(story)) {
        let Some(date) = story_completion_date(story) else {
            continue;
        };
        *completed_by_date.entry(date).or_default() += story.story_points.unwrap_or(0);
    }

    let mut scope_changes = BTreeMap::<NaiveDate, i64>::new();
    let mut sprint_boundaries = BTreeSet::<NaiveDate>::new();
    for sprint in sprints {
        let Some(start_date) = sprint.start_date.as_deref().and_then(parse_date_prefix) else {
            continue;
        };
        if start_date > today {
            continue;
        }
        sprint_boundaries.insert(start_date);
        if let Some(end_date) = sprint.end_date.as_deref().and_then(parse_date_prefix)
            && end_date <= today
        {
            sprint_boundaries.insert(end_date);
        }
        *scope_changes.entry(start_date).or_default() += sprint_total_points(sprint);
    }

    let start_date = stories
        .iter()
        .filter_map(story_work_started_date)
        .min()
        .or_else(|| completed_by_date.keys().next().copied())
        .or_else(|| scope_changes.keys().next().copied());
    let Some(start_date) = start_date else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    let mut cumulative = completed_by_date
        .range(..start_date)
        .map(|(_, points)| *points)
        .sum::<i64>();
    let mut scope = scope_changes
        .range(..=start_date)
        .map(|(_, points)| *points)
        .sum::<i64>();
    rows.push(BurnupPoint {
        date: start_date.to_string(),
        completed: cumulative,
        scope,
    });

    let dates = completed_by_date
        .keys()
        .chain(scope_changes.keys())
        .chain(sprint_boundaries.iter())
        .copied()
        .filter(|date| *date >= start_date && *date <= today)
        .collect::<BTreeSet<_>>();
    for date in dates {
        if date > start_date {
            scope += scope_changes.get(&date).copied().unwrap_or(0);
            cumulative += completed_by_date.get(&date).copied().unwrap_or(0);
            rows.push(BurnupPoint {
                date: date.to_string(),
                completed: cumulative,
                scope,
            });
            continue;
        }
        cumulative += completed_by_date.get(&date).copied().unwrap_or(0);
        if let Some(last) = rows.last_mut() {
            last.completed = cumulative;
            last.scope = scope;
        }
    }

    if rows
        .last()
        .is_some_and(|last| last.date != today.to_string())
    {
        rows.push(BurnupPoint {
            date: today.to_string(),
            completed: cumulative,
            scope,
        });
    }
    rows
}

pub(crate) fn build_burndown(sprints: &[WebSprint]) -> Vec<BurndownPoint> {
    let Some(sprint) = select_burndown_sprint(sprints) else {
        return Vec::new();
    };
    let Some(start_date) = sprint.start_date.as_deref().and_then(parse_date_prefix) else {
        return Vec::new();
    };
    let Some(end_date) = sprint.end_date.as_deref().and_then(parse_date_prefix) else {
        return Vec::new();
    };
    if end_date < start_date {
        return Vec::new();
    }

    let planned_points = sprint_total_points(sprint);
    if planned_points <= 0 {
        return Vec::new();
    }

    let today = Local::now().date_naive();
    let last_date = match sprint.status.as_deref() {
        Some("closed") => end_date,
        _ => std::cmp::min(end_date, std::cmp::max(start_date, today)),
    };

    let mut completed_by_date = BTreeMap::<NaiveDate, i64>::new();
    for story in sprint.stories_by_status.values().flatten() {
        if !story_is_done(story) {
            continue;
        }
        let Some(date) = story_completion_date(story) else {
            continue;
        };
        *completed_by_date
            .entry(std::cmp::min(date, end_date))
            .or_default() += story.story_points.unwrap_or(0);
    }

    let total_days = (end_date - start_date).num_days();
    let visible_days = (last_date - start_date).num_days();
    let mut rows = Vec::new();
    let mut completed = 0;
    for offset in 0..=visible_days {
        let date = start_date + Days::new(offset as u64);
        completed += completed_by_date.get(&date).copied().unwrap_or(0);
        let remaining = (planned_points - completed).max(0);
        let ideal = if total_days <= 0 {
            0
        } else {
            (((planned_points as f64) * (1.0 - (offset as f64 / total_days as f64))).round() as i64)
                .max(0)
        };
        rows.push(BurndownPoint {
            date: date.to_string(),
            remaining,
            ideal,
        });
    }
    rows
}

fn select_burndown_sprint(sprints: &[WebSprint]) -> Option<&WebSprint> {
    sprints
        .iter()
        .find(|sprint| sprint.status.as_deref() == Some("active"))
        .or_else(|| {
            sprints.iter().rev().find(|sprint| {
                sprint.status.as_deref() != Some("closed") && sprint_total_points(sprint) > 0
            })
        })
        .or_else(|| {
            sprints
                .iter()
                .rev()
                .find(|sprint| sprint_total_points(sprint) > 0)
        })
        .or_else(|| sprints.last())
}

fn sprint_total_points(sprint: &WebSprint) -> i64 {
    sprint
        .stories_by_status
        .values()
        .flatten()
        .filter(|story| story_counts_toward_scope(story))
        .map(|story| story.story_points.unwrap_or(0))
        .sum()
}

fn story_work_started_date(story: &WebStory) -> Option<NaiveDate> {
    story.work_started.as_deref().and_then(parse_date_prefix)
}

fn story_completion_date(story: &WebStory) -> Option<NaiveDate> {
    story
        .work_done
        .as_deref()
        .and_then(parse_date_prefix)
        .or_else(|| story.updated.as_deref().and_then(parse_date_prefix))
        .or_else(|| story.created.as_deref().and_then(parse_date_prefix))
}

pub(crate) fn build_lead_time(stories: &[WebStory]) -> Vec<LeadTimePoint> {
    let mut done = stories
        .iter()
        .filter(|story| {
            story_is_done(story) && story.work_started.is_some() && story.work_done.is_some()
        })
        .collect::<Vec<_>>();
    done.sort_by(|a, b| a.work_done.cmp(&b.work_done));
    let mut window = Vec::<i64>::new();
    let mut points = Vec::new();
    for story in done {
        let days = days_between(
            story.work_started.as_deref().unwrap_or_default(),
            story.work_done.as_deref().unwrap_or_default(),
        )
        .unwrap_or(0);
        window.push(days);
        if window.len() > 7 {
            window.remove(0);
        }
        let rolling_avg = window.iter().sum::<i64>() as f64 / window.len() as f64;
        points.push(LeadTimePoint {
            story_id: story.id.clone(),
            date: story.work_done.clone().unwrap_or_default(),
            days,
            rolling_avg,
        });
    }
    points
}

pub(crate) fn build_velocity(sprints: &[WebSprint]) -> Vec<VelocityPoint> {
    sprints
        .iter()
        .map(|sprint| VelocityPoint {
            sprint: sprint.name.clone(),
            points: sprint
                .stories_by_status
                .get(StoryStatus::Done.as_str())
                .map(|stories| {
                    stories
                        .iter()
                        .filter(|story| story_counts_toward_scope(story))
                        .map(|story| story.story_points.unwrap_or(0))
                        .sum()
                })
                .unwrap_or(0),
            forecast: false,
        })
        .collect()
}

pub(crate) fn build_forecast(stories: &[StoryOverview], sprints: &[SprintOverview]) -> Forecast {
    let current_sprint_name = sprints
        .iter()
        .find(|sprint| sprint.readme_status.as_deref() == Some("active"))
        .map(|sprint| sprint.sprint_name.as_str());
    let dto = ReportForecastDto::build(stories, sprints, current_sprint_name);
    Forecast::from(dto)
}

impl From<ReportForecastDto> for Forecast {
    fn from(dto: ReportForecastDto) -> Self {
        Forecast {
            generated_at: dto.generated_at,
            remaining_points: dto.remaining_points,
            sprint_duration_weeks: dto.sprint_duration_weeks as i64,
            projection_start_date: dto.projection_start_date,
            throughput: ForecastThroughput {
                samples: dto.throughput.samples,
                average: dto.throughput.average,
                median: dto.throughput.median,
                observed_day_count: dto.throughput.observed_day_count,
            },
            completion: ForecastCompletion {
                p50_days: dto.completion.p50_days.map(i64::from),
                p80_days: dto.completion.p80_days.map(i64::from),
                p90_days: dto.completion.p90_days.map(i64::from),
                p50_date: dto.completion.p50_date,
                p80_date: dto.completion.p80_date,
                p90_date: dto.completion.p90_date,
            },
            confidence: dto.confidence,
        }
    }
}

fn days_between(start: &str, end: &str) -> Option<i64> {
    let start = parse_date_prefix(start)?;
    let end = parse_date_prefix(end)?;
    Some((end - start).num_days().max(0))
}

fn parse_date_prefix(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests;
