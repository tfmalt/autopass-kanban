use chrono::{Datelike, Days};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::{EpicOverview, SprintOverview, StoryOverview, StoryStatus};

use super::{ForecastInputs, ReportForecastDto, parse_points, path_string};

fn phase_from_story_id(id: &str) -> String {
    id.split('-').nth(1).unwrap_or("unknown").to_string()
}

fn normalize_story_status(status: &str) -> String {
    super::slugify_status(status)
}

fn status_label(status: &str) -> String {
    match status {
        "draft" => "DRAFT".to_string(),
        "ready" => "READY".to_string(),
        "planned" => "PLANNED".to_string(),
        "todo" => "TODO".to_string(),
        "in-progress" => "IN PROGRESS".to_string(),
        "ready-for-qa" => "READY FOR QA".to_string(),
        "blocked" => "BLOCKED".to_string(),
        "done" => "DONE".to_string(),
        "dropped" => "DROPPED".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

#[derive(Debug, Clone, Copy)]
struct PhaseReportMeta {
    title: &'static str,
    milestone: &'static str,
    period: &'static str,
    priority: &'static str,
}

fn phase_report_meta(phase: &str) -> PhaseReportMeta {
    match phase {
        "F1" => PhaseReportMeta {
            title: "Phase 1 - Etablering (Establishment)",
            milestone: "MP1 - Foundation",
            period: "Q2 2026",
            priority: "Critical",
        },
        "F2" => PhaseReportMeta {
            title: "Phase 2 - Utvikling: Kjernelogikk (Core Logic)",
            milestone: "MP2 - Core Logic",
            period: "Q3 2026",
            priority: "Critical",
        },
        "F3" => PhaseReportMeta {
            title: "Phase 3 - Utvikling: Administrasjon (Admin)",
            milestone: "MP3 - Administration",
            period: "Q4 2026",
            priority: "High",
        },
        "F4" => PhaseReportMeta {
            title: "Phase 4 - Utvikling: Ferdigstillelse (Completion)",
            milestone: "MP4 - Complete Functionality",
            period: "Q1 2027",
            priority: "High",
        },
        "F5" => PhaseReportMeta {
            title: "Phase 5 - Driftssettelse og Stabilisering",
            milestone: "MP5 - Production Readiness",
            period: "Q2 2027",
            priority: "High",
        },
        _ => PhaseReportMeta {
            title: "",
            milestone: "",
            period: "",
            priority: "",
        },
    }
}

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn parse_date_prefix(value: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()
}

fn date_string(date: chrono::NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn quarter(date: chrono::NaiveDate) -> (u32, i32) {
    ((date.month() - 1) / 3 + 1, date.year())
}

fn period_label(
    start: Option<chrono::NaiveDate>,
    end: Option<chrono::NaiveDate>,
) -> Option<String> {
    let first = start.or(end)?;
    let last = end.or(start)?;
    let (q1, y1) = quarter(first);
    let (q2, y2) = quarter(last);
    if (q1, y1) == (q2, y2) {
        Some(format!("Q{q1} {y1}"))
    } else if y1 == y2 {
        Some(format!("Q{q1}-Q{q2} {y1}"))
    } else {
        Some(format!("Q{q1} {y1}-Q{q2} {y2}"))
    }
}

fn is_weekday(date: chrono::NaiveDate) -> bool {
    date.weekday().number_from_monday() <= 5
}

fn add_working_days(date: chrono::NaiveDate, days: f64) -> chrono::NaiveDate {
    let mut current = date;
    let mut remaining = days;
    while remaining > 0.0 {
        current += chrono::Duration::days(1);
        if is_weekday(current) {
            remaining -= 1.0;
        }
    }
    current
}

fn work_days_inclusive(start: chrono::NaiveDate, end: chrono::NaiveDate) -> i64 {
    let mut days = 0;
    let mut cursor = start;
    while cursor <= end {
        if is_weekday(cursor) {
            days += 1;
        }
        cursor += chrono::Duration::days(1);
    }
    days
}

fn story_points(story: &StoryOverview) -> i64 {
    parse_points(&story.story_points).unwrap_or(0)
}

fn story_counts_toward_scope(story: &StoryOverview) -> bool {
    StoryStatus::parse_counts_toward_scope(&story.status)
}

fn story_is_done(story: &StoryOverview) -> bool {
    StoryStatus::parse(&story.status) == Some(StoryStatus::Done)
}

fn story_is_terminal(story: &StoryOverview) -> bool {
    StoryStatus::parse_is_terminal(&story.status)
}

fn sum_points<'a>(stories: impl IntoIterator<Item = &'a StoryOverview>) -> i64 {
    stories
        .into_iter()
        .filter(|story| story_counts_toward_scope(story))
        .map(story_points)
        .sum()
}

fn sprint_story_id(sprint_name: &str, story_id: &str) -> String {
    format!("{sprint_name}\0{story_id}")
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

/// Per-story estimate row used by the web report endpoint.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportEstimateDto {
    pub story_id: String,
    pub est_hours: Option<f64>,
    pub est_start: Option<String>,
    pub est_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportWbsRowDto {
    pub kind: String,
    pub wbs: String,
    pub id: String,
    pub title: String,
    pub milestone: String,
    pub period: String,
    pub priority: String,
    pub status: String,
    pub points: Option<i64>,
    pub est_hours: Option<f64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportPhaseRowDto {
    pub phase: String,
    pub title: String,
    pub period: String,
    pub milestone: String,
    pub epics: usize,
    pub stories: usize,
    pub total: i64,
    pub done: i64,
    pub wip: i64,
    pub remaining: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportSprintProjectionDto {
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub planned_points: Option<i64>,
    pub delivered_points: Option<i64>,
    pub rate: Option<f64>,
    pub remaining: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportWorkbookRowDto {
    pub kind: String,
    pub wbs: String,
    pub id: String,
    pub title: String,
    pub milestone: String,
    pub priority: String,
    pub status: String,
    pub points: Option<i64>,
    pub est_hours: Option<f64>,
    pub planned_period: Option<String>,
    pub planned_start_date: Option<String>,
    pub planned_end_date: Option<String>,
    pub actual_period: Option<String>,
    pub actual_start_date: Option<String>,
    pub actual_end_date: Option<String>,
    pub completed_in_sprint: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportProgressDto {
    pub done_points: i64,
    pub total_points: i64,
    pub done_stories: usize,
    pub total_stories: usize,
}

/// Derived report rows used by the local web app report view.
#[derive(Debug, Clone, Serialize)]
pub struct ReportDashboardDto {
    pub generated_at: String,
    pub daily_avg: f64,
    pub throughput_source: String,
    pub hours_per_point: f64,
    pub remaining_points: i64,
    pub progress: ReportProgressDto,
    pub forecast: ReportForecastDto,
    pub estimates: Vec<ReportEstimateDto>,
    pub wbs_rows: Vec<ReportWbsRowDto>,
    pub phase_rows: Vec<ReportPhaseRowDto>,
    pub sprint_rows: Vec<ReportSprintProjectionDto>,
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
    pub daily_avg: f64,
    pub throughput_source: String,
    pub hours_per_point: f64,
    pub wbs_rows: Vec<ReportWorkbookRowDto>,
    pub phase_rows: Vec<ReportPhaseRowDto>,
    pub sprint_rows: Vec<ReportSprintProjectionDto>,
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

#[derive(Debug, Clone)]
struct EstimateContext {
    estimates: BTreeMap<String, ReportEstimateDto>,
    daily_avg: f64,
    throughput_source: String,
    hours_per_point: f64,
}

struct DerivedReportRows {
    estimate_context: EstimateContext,
    web_wbs_rows: Vec<ReportWbsRowDto>,
    workbook_rows: Vec<ReportWorkbookRowDto>,
    phase_rows: Vec<ReportPhaseRowDto>,
    sprint_rows: Vec<ReportSprintProjectionDto>,
    progress: ReportProgressDto,
}

#[derive(Debug, Clone)]
struct PhaseGroup<'a> {
    id: String,
    epics: Vec<EpicGroup<'a>>,
}

#[derive(Debug, Clone)]
struct EpicGroup<'a> {
    id: String,
    title: String,
    stories: Vec<&'a StoryOverview>,
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
        Self::build_with_epics(stories, sprints, &[], current_sprint_name)
    }

    pub fn build_with_epics(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        epics: &[EpicOverview],
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
        let derived = derive_report_rows(stories, sprints, Some(epics), &forecast);

        ReportWbsDto {
            generated_at,
            stories: prepared.stories,
            sprints: prepared.sprints,
            phases: prepared.phases,
            velocity: prepared.velocity,
            forecast,
            daily_avg: derived.estimate_context.daily_avg,
            throughput_source: derived.estimate_context.throughput_source,
            hours_per_point: derived.estimate_context.hours_per_point,
            wbs_rows: derived.workbook_rows,
            phase_rows: derived.phase_rows,
            sprint_rows: derived.sprint_rows,
        }
    }
}

impl ReportDashboardDto {
    pub fn build(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        current_sprint_name: Option<&str>,
    ) -> Self {
        use chrono::Local;

        let today = Local::now().date_naive();
        let generated_at = Local::now().to_rfc3339();
        Self::build_with_context(stories, sprints, current_sprint_name, generated_at, today)
    }

    fn build_with_context(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        current_sprint_name: Option<&str>,
        generated_at: String,
        today: chrono::NaiveDate,
    ) -> Self {
        let prepared = PreparedReport::build(
            stories,
            sprints,
            current_sprint_name,
            generated_at.clone(),
            today,
        );
        let forecast = ReportForecastDto::from_inputs(prepared.forecast_inputs);
        let derived = derive_report_rows(stories, sprints, None, &forecast);

        Self {
            generated_at,
            daily_avg: derived.estimate_context.daily_avg,
            throughput_source: derived.estimate_context.throughput_source.clone(),
            hours_per_point: derived.estimate_context.hours_per_point,
            remaining_points: forecast.remaining_points,
            progress: derived.progress,
            forecast,
            estimates: derived
                .estimate_context
                .estimates
                .values()
                .cloned()
                .collect(),
            wbs_rows: derived.web_wbs_rows,
            phase_rows: derived.phase_rows,
            sprint_rows: derived.sprint_rows,
        }
    }
}

fn derive_report_rows(
    stories: &[StoryOverview],
    sprints: &[SprintOverview],
    epics: Option<&[EpicOverview]>,
    forecast: &ReportForecastDto,
) -> DerivedReportRows {
    let estimate_context = build_estimates(stories, sprints, forecast);
    let (web_wbs_rows, workbook_rows) = build_wbs_rows(stories, sprints, epics, &estimate_context);
    let phase_rows = build_phase_rows(stories);
    let sprint_rows = build_sprint_rows(sprints, forecast, &estimate_context);
    let progress = build_progress(stories);
    DerivedReportRows {
        estimate_context,
        web_wbs_rows,
        workbook_rows,
        phase_rows,
        sprint_rows,
        progress,
    }
}

fn throughput_source(sprints: &[SprintOverview], forecast: &ReportForecastDto) -> (f64, String) {
    let daily_avg = forecast.throughput.average;
    let observed = forecast.throughput.observed_day_count;
    if daily_avg > 0.0 {
        return (
            daily_avg,
            format!("daily throughput over {observed} observed workdays"),
        );
    }

    let projection_start = parse_date_prefix(&forecast.projection_start_date);
    let delivered = sprints
        .iter()
        .filter(|sprint| {
            projection_start.is_some_and(|start| {
                parse_date_prefix(&sprint.end_date).is_some_and(|end| end < start)
            })
        })
        .map(done_points_in_sprint)
        .collect::<Vec<_>>();
    let avg_sprint = average(&delivered);
    let daily_fallback = avg_sprint / (forecast.sprint_duration_weeks * 5).max(1) as f64;
    if daily_fallback > 0.0 {
        (daily_fallback, "sprint velocity fallback".to_string())
    } else {
        (0.0, "no throughput data".to_string())
    }
}

fn build_estimates(
    stories: &[StoryOverview],
    sprints: &[SprintOverview],
    forecast: &ReportForecastDto,
) -> EstimateContext {
    let (daily_avg, throughput_source) = throughput_source(sprints, forecast);
    let mut estimates = BTreeMap::new();
    if daily_avg <= 0.0 {
        for story in stories {
            estimates.insert(
                story.id.clone(),
                ReportEstimateDto {
                    story_id: story.id.clone(),
                    est_hours: None,
                    est_start: None,
                    est_end: None,
                },
            );
        }
        return EstimateContext {
            estimates,
            daily_avg,
            throughput_source,
            hours_per_point: 0.0,
        };
    }

    let today = parse_date_prefix(&forecast.projection_start_date)
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    let hours_per_point = 7.0 / daily_avg;
    let days_per_point = 1.0 / daily_avg;
    let mut cumulative_days = 0.0;
    let mut remaining_stories = stories
        .iter()
        .filter(|story| !story_is_terminal(story))
        .collect::<Vec<_>>();
    remaining_stories.sort_by_key(|story| estimate_sort_key(story));

    for story in remaining_stories {
        let points = story_points(story);
        if points <= 0 {
            estimates.insert(
                story.id.clone(),
                ReportEstimateDto {
                    story_id: story.id.clone(),
                    est_hours: None,
                    est_start: None,
                    est_end: None,
                },
            );
            continue;
        }

        let est_hours = Some(round_metric(points as f64 * hours_per_point));
        let duration = points as f64 * days_per_point;
        if let Some(work_started) = story.work_started.as_deref().and_then(parse_date_prefix) {
            estimates.insert(
                story.id.clone(),
                ReportEstimateDto {
                    story_id: story.id.clone(),
                    est_hours,
                    est_start: Some(date_string(work_started)),
                    est_end: Some(date_string(add_working_days(today, duration))),
                },
            );
        } else {
            estimates.insert(
                story.id.clone(),
                ReportEstimateDto {
                    story_id: story.id.clone(),
                    est_hours,
                    est_start: Some(date_string(add_working_days(today, cumulative_days))),
                    est_end: Some(date_string(add_working_days(
                        today,
                        cumulative_days + duration,
                    ))),
                },
            );
            cumulative_days += duration;
        }
    }

    for story in stories {
        estimates
            .entry(story.id.clone())
            .or_insert_with(|| ReportEstimateDto {
                story_id: story.id.clone(),
                est_hours: None,
                est_start: story
                    .work_started
                    .as_deref()
                    .and_then(parse_date_prefix)
                    .map(date_string),
                est_end: story
                    .work_done
                    .as_deref()
                    .and_then(parse_date_prefix)
                    .map(date_string),
            });
    }

    EstimateContext {
        estimates,
        daily_avg,
        throughput_source,
        hours_per_point,
    }
}

fn estimate_sort_key(story: &StoryOverview) -> (u8, String, String, String) {
    let status_rank = match normalize_story_status(&story.status).as_str() {
        "in-progress" => 0,
        "ready-for-qa" => 1,
        "todo" => 2,
        "planned" => 3,
        "ready" => 4,
        "draft" => 5,
        "blocked" => 6,
        _ => 9,
    };
    (
        status_rank,
        phase_from_story_id(&story.id),
        story.epic_id.clone().unwrap_or_default(),
        story.id.clone(),
    )
}

fn build_hierarchy(stories: &[StoryOverview]) -> Vec<PhaseGroup<'_>> {
    let mut phase_map = BTreeMap::<String, BTreeMap<String, EpicGroup<'_>>>::new();
    for story in stories {
        let phase = phase_from_story_id(&story.id);
        let epic_id = story
            .epic_id
            .clone()
            .unwrap_or_else(|| format!("(no epic in {phase})"));
        let epic_map = phase_map.entry(phase).or_default();
        let epic = epic_map
            .entry(epic_id.clone())
            .or_insert_with(|| EpicGroup {
                id: epic_id.clone(),
                title: story.epic_title.clone().unwrap_or(epic_id),
                stories: Vec::new(),
            });
        epic.stories.push(story);
    }

    phase_map
        .into_iter()
        .map(|(id, epics)| {
            let mut epics = epics.into_values().collect::<Vec<_>>();
            for epic in &mut epics {
                epic.stories.sort_by(|left, right| left.id.cmp(&right.id));
            }
            PhaseGroup { id, epics }
        })
        .collect()
}

fn group_dates(
    stories: &[&StoryOverview],
    estimates: &BTreeMap<String, ReportEstimateDto>,
) -> (Option<String>, Option<String>) {
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    for story in stories {
        let status = normalize_story_status(&story.status);
        let started = story
            .work_started
            .as_deref()
            .and_then(parse_date_prefix)
            .map(date_string);
        let done = story
            .work_done
            .as_deref()
            .and_then(parse_date_prefix)
            .map(date_string);
        let estimate = estimates.get(&story.id);
        if status == "done" || status == "dropped" {
            if let Some(started) = started {
                starts.push(started);
            }
            if let Some(done) = done {
                ends.push(done);
            }
        } else if status == "in-progress" || status == "ready-for-qa" {
            if let Some(started) = started {
                starts.push(started);
            }
            if let Some(end) = estimate.and_then(|estimate| estimate.est_end.clone()) {
                ends.push(end);
            }
        } else {
            if let Some(start) = estimate.and_then(|estimate| estimate.est_start.clone()) {
                starts.push(start);
            }
            if let Some(end) = estimate.and_then(|estimate| estimate.est_end.clone()) {
                ends.push(end);
            }
        }
    }
    starts.sort();
    ends.sort();
    (starts.first().cloned(), ends.last().cloned())
}

fn group_planned_dates(
    stories: &[&StoryOverview],
) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    let starts = stories
        .iter()
        .filter_map(|story| story.planned_start.as_deref().and_then(parse_date_prefix))
        .collect::<Vec<_>>();
    let ends = stories
        .iter()
        .filter_map(|story| story.planned_end.as_deref().and_then(parse_date_prefix))
        .collect::<Vec<_>>();
    (starts.into_iter().min(), ends.into_iter().max())
}

fn group_actual_dates(
    stories: &[&StoryOverview],
) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    let starts = stories
        .iter()
        .filter_map(|story| story.work_started.as_deref().and_then(parse_date_prefix))
        .collect::<Vec<_>>();
    let ends = stories
        .iter()
        .filter_map(|story| story.work_done.as_deref().and_then(parse_date_prefix))
        .collect::<Vec<_>>();
    (starts.into_iter().min(), ends.into_iter().max())
}

fn aggregate_status(stories: &[&StoryOverview]) -> String {
    let statuses = stories
        .iter()
        .map(|story| normalize_story_status(&story.status))
        .filter(|status| !status.is_empty())
        .collect::<BTreeSet<_>>();
    if statuses.is_empty() {
        return "PLANNED".to_string();
    }
    if statuses
        .iter()
        .all(|status| matches!(status.as_str(), "done" | "dropped"))
    {
        return "DONE".to_string();
    }
    if statuses
        .iter()
        .any(|status| matches!(status.as_str(), "in-progress" | "ready-for-qa" | "blocked"))
    {
        return "IN PROGRESS".to_string();
    }
    if statuses
        .iter()
        .any(|status| matches!(status.as_str(), "done" | "dropped"))
    {
        return "IN PROGRESS".to_string();
    }
    if statuses.iter().any(|status| status == "todo") {
        return "TODO".to_string();
    }
    "PLANNED".to_string()
}

fn maybe_date_string(date: Option<chrono::NaiveDate>) -> Option<String> {
    date.map(date_string)
}

fn completion_sprint_for_date(
    date: Option<chrono::NaiveDate>,
    sprints: &[SprintOverview],
) -> Option<String> {
    let date = date?;
    sprints
        .iter()
        .filter(|sprint| {
            let start = parse_date_prefix(&sprint.start_date);
            let end = parse_date_prefix(&sprint.end_date);
            start.is_some_and(|start| date >= start) && end.is_some_and(|end| date <= end)
        })
        .min_by(|left, right| {
            left.sprint_name
                .cmp(&right.sprint_name)
                .then_with(|| left.start_date.cmp(&right.start_date))
        })
        .map(|sprint| sprint.sprint_name.clone())
}

fn epic_completion_sprint(
    epic_id: &str,
    epics: Option<&[EpicOverview]>,
    sprints: &[SprintOverview],
) -> Option<String> {
    let epic = epics?
        .iter()
        .find(|epic| epic.id.eq_ignore_ascii_case(epic_id))?;
    completion_sprint_for_date(
        epic.work_done.as_deref().and_then(parse_date_prefix),
        sprints,
    )
}

fn epic_completion_note(
    epic_id: &str,
    epics: Option<&[EpicOverview]>,
    sprints: &[SprintOverview],
) -> String {
    let Some(epics) = epics else {
        return String::new();
    };
    let Some(epic) = epics
        .iter()
        .find(|epic| epic.id.eq_ignore_ascii_case(epic_id))
    else {
        return String::new();
    };
    let is_terminal = matches!(
        normalize_story_status(&epic.status).as_str(),
        "done" | "dropped"
    );
    if is_terminal && epic_completion_sprint(epic_id, Some(epics), sprints).is_none() {
        "Unable to resolve completion sprint from epic work_done and known sprint ranges."
            .to_string()
    } else {
        String::new()
    }
}

#[allow(clippy::too_many_arguments)]
fn build_group_workbook_row(
    kind: &str,
    wbs: String,
    id: String,
    title: String,
    meta: PhaseReportMeta,
    stories: &[&StoryOverview],
    points: i64,
    completed_in_sprint: Option<String>,
    notes: String,
) -> ReportWorkbookRowDto {
    let (planned_start, planned_end) = group_planned_dates(stories);
    let (actual_start, actual_end) = group_actual_dates(stories);
    ReportWorkbookRowDto {
        kind: kind.to_string(),
        wbs,
        id,
        title,
        milestone: meta.milestone.to_string(),
        priority: meta.priority.to_string(),
        status: aggregate_status(stories),
        points: Some(points),
        est_hours: None,
        planned_period: period_label(planned_start, planned_end).or_else(|| {
            if meta.period.is_empty() {
                None
            } else {
                Some(meta.period.to_string())
            }
        }),
        planned_start_date: maybe_date_string(planned_start),
        planned_end_date: maybe_date_string(planned_end),
        actual_period: period_label(actual_start, actual_end),
        actual_start_date: maybe_date_string(actual_start),
        actual_end_date: maybe_date_string(actual_end),
        completed_in_sprint,
        notes,
    }
}

fn build_story_workbook_row(
    wbs: String,
    story: &StoryOverview,
    meta: PhaseReportMeta,
    estimates: &EstimateContext,
) -> ReportWorkbookRowDto {
    let status = normalize_story_status(&story.status);
    let points = parse_points(&story.story_points);
    let estimate = estimates.estimates.get(&story.id);
    let active_or_done = matches!(
        status.as_str(),
        "done" | "dropped" | "in-progress" | "ready-for-qa"
    );
    let est_hours = if active_or_done
        && points.is_some_and(|points| points != 0)
        && estimates.hours_per_point > 0.0
    {
        Some(round_metric(
            points.unwrap_or(0) as f64 * estimates.hours_per_point,
        ))
    } else {
        estimate.and_then(|estimate| estimate.est_hours)
    };
    let planned_start = story.planned_start.as_deref().and_then(parse_date_prefix);
    let planned_end = story.planned_end.as_deref().and_then(parse_date_prefix);
    let actual_start = story.work_started.as_deref().and_then(parse_date_prefix);
    let actual_end = story.work_done.as_deref().and_then(parse_date_prefix);
    let mut missing = Vec::new();
    if planned_start.is_none() {
        missing.push("start");
    }
    if planned_end.is_none() {
        missing.push("end");
    }

    let mut notes = if missing.is_empty() {
        String::new()
    } else {
        format!("Missing planned baseline: {}", missing.join(", "))
    };
    let completed_in_sprint = if story_is_terminal(story) {
        story.sprint.clone()
    } else {
        None
    };
    if story_is_terminal(story) && completed_in_sprint.is_none() {
        if !notes.is_empty() {
            notes.push_str("; ");
        }
        notes.push_str(
            "Unable to resolve completion sprint: terminal story has no retained sprint.",
        );
    }

    ReportWorkbookRowDto {
        kind: "story".to_string(),
        wbs,
        id: story.id.clone(),
        title: story.title.clone(),
        milestone: meta.milestone.to_string(),
        priority: meta.priority.to_string(),
        status: status_label(&status),
        points,
        est_hours,
        planned_period: period_label(planned_start, planned_end),
        planned_start_date: maybe_date_string(planned_start),
        planned_end_date: maybe_date_string(planned_end),
        actual_period: period_label(actual_start, actual_end),
        actual_start_date: maybe_date_string(actual_start),
        actual_end_date: maybe_date_string(actual_end),
        completed_in_sprint,
        notes,
    }
}

fn build_wbs_rows(
    stories: &[StoryOverview],
    sprints: &[SprintOverview],
    epics: Option<&[EpicOverview]>,
    estimates: &EstimateContext,
) -> (Vec<ReportWbsRowDto>, Vec<ReportWorkbookRowDto>) {
    let mut web_rows = Vec::new();
    let mut workbook_rows = Vec::new();
    for (phase_index, phase) in build_hierarchy(stories).into_iter().enumerate() {
        let phase_wbs = (phase_index + 1).to_string();
        let meta = phase_report_meta(&phase.id);
        let phase_title = if meta.title.is_empty() {
            phase.id.clone()
        } else {
            meta.title.to_string()
        };
        let phase_stories = phase
            .epics
            .iter()
            .flat_map(|epic| epic.stories.iter().copied())
            .collect::<Vec<_>>();
        let phase_dates = group_dates(&phase_stories, &estimates.estimates);
        web_rows.push(ReportWbsRowDto {
            kind: "phase".to_string(),
            wbs: phase_wbs.clone(),
            id: phase.id.clone(),
            title: phase_title.clone(),
            milestone: meta.milestone.to_string(),
            period: meta.period.to_string(),
            priority: meta.priority.to_string(),
            status: String::new(),
            points: Some(sum_points(phase_stories.iter().copied())),
            est_hours: None,
            start_date: phase_dates.0,
            end_date: phase_dates.1,
            notes: String::new(),
        });
        workbook_rows.push(build_group_workbook_row(
            "phase",
            phase_wbs.clone(),
            phase.id.clone(),
            phase_title,
            meta,
            &phase_stories,
            sum_points(phase_stories.iter().copied()),
            None,
            String::new(),
        ));

        for (epic_index, epic) in phase.epics.into_iter().enumerate() {
            let epic_wbs = format!("{phase_wbs}.{}", epic_index + 1);
            let epic_dates = group_dates(&epic.stories, &estimates.estimates);
            web_rows.push(ReportWbsRowDto {
                kind: "epic".to_string(),
                wbs: epic_wbs.clone(),
                id: epic.id.clone(),
                title: epic.title.clone(),
                milestone: meta.milestone.to_string(),
                period: meta.period.to_string(),
                priority: meta.priority.to_string(),
                status: String::new(),
                points: Some(sum_points(epic.stories.iter().copied())),
                est_hours: None,
                start_date: epic_dates.0,
                end_date: epic_dates.1,
                notes: String::new(),
            });
            workbook_rows.push(build_group_workbook_row(
                "epic",
                epic_wbs.clone(),
                epic.id.clone(),
                epic.title.clone(),
                meta,
                &epic.stories,
                sum_points(epic.stories.iter().copied()),
                epic_completion_sprint(&epic.id, epics, sprints),
                epic_completion_note(&epic.id, epics, sprints),
            ));

            for (story_index, story) in epic.stories.into_iter().enumerate() {
                let story_wbs = format!("{epic_wbs}.{}", story_index + 1);
                let status = normalize_story_status(&story.status);
                let estimate = estimates.estimates.get(&story.id);
                let active_or_done =
                    matches!(status.as_str(), "done" | "in-progress" | "ready-for-qa");
                let points = parse_points(&story.story_points);
                let est_hours = if active_or_done
                    && points.is_some_and(|points| points != 0)
                    && estimates.hours_per_point > 0.0
                {
                    Some(round_metric(
                        points.unwrap_or(0) as f64 * estimates.hours_per_point,
                    ))
                } else {
                    estimate.and_then(|estimate| estimate.est_hours)
                };
                let started = story
                    .work_started
                    .as_deref()
                    .and_then(parse_date_prefix)
                    .map(date_string);
                let done = story
                    .work_done
                    .as_deref()
                    .and_then(parse_date_prefix)
                    .map(date_string);
                let start_date = if status == "done" || status == "dropped" || active_or_done {
                    started.or_else(|| estimate.and_then(|estimate| estimate.est_start.clone()))
                } else {
                    estimate.and_then(|estimate| estimate.est_start.clone())
                };
                let end_date = if status == "done" || status == "dropped" {
                    done
                } else {
                    estimate.and_then(|estimate| estimate.est_end.clone())
                };
                let notes = [
                    story
                        .sprint
                        .as_ref()
                        .map(|sprint| format!("Sprint {sprint}")),
                    if story.assignee.trim().is_empty() {
                        None
                    } else {
                        Some(format!("Assignee {}", story.assignee))
                    },
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; ");

                web_rows.push(ReportWbsRowDto {
                    kind: "story".to_string(),
                    wbs: story_wbs.clone(),
                    id: story.id.clone(),
                    title: story.title.clone(),
                    milestone: meta.milestone.to_string(),
                    period: meta.period.to_string(),
                    priority: meta.priority.to_string(),
                    status: status_label(&status),
                    points: if status == "dropped" { None } else { points },
                    est_hours,
                    start_date,
                    end_date,
                    notes,
                });
                workbook_rows.push(build_story_workbook_row(story_wbs, story, meta, estimates));
            }
        }
    }
    (web_rows, workbook_rows)
}

fn build_progress(stories: &[StoryOverview]) -> ReportProgressDto {
    let mut done_points = 0;
    let mut total_points = 0;
    let mut done_stories = 0;
    let mut total_stories = 0;
    for story in stories {
        let points = story_points(story);
        if story_counts_toward_scope(story) {
            total_points += points;
            total_stories += 1;
        }
        if story_is_done(story) {
            done_points += points;
            done_stories += 1;
        }
    }
    ReportProgressDto {
        done_points,
        total_points,
        done_stories,
        total_stories,
    }
}

fn build_phase_rows(stories: &[StoryOverview]) -> Vec<ReportPhaseRowDto> {
    let mut phases = BTreeMap::<String, (BTreeSet<String>, usize, i64, i64, i64)>::new();
    for story in stories {
        let phase = phase_from_story_id(&story.id);
        let entry = phases.entry(phase).or_default();
        entry
            .0
            .insert(story.epic_id.clone().unwrap_or_else(|| "?".to_string()));
        let points = story_points(story);
        if story_counts_toward_scope(story) {
            entry.1 += 1;
            entry.2 += points;
        }
        if story_is_done(story) {
            entry.3 += points;
        }
        let status = normalize_story_status(&story.status);
        if status == "in-progress" || status == "ready-for-qa" {
            entry.4 += points;
        }
    }

    phases
        .into_iter()
        .map(|(phase, (epics, stories, total, done, wip))| {
            let meta = phase_report_meta(&phase);
            ReportPhaseRowDto {
                title: if meta.title.is_empty() {
                    phase.clone()
                } else {
                    meta.title.to_string()
                },
                period: meta.period.to_string(),
                milestone: meta.milestone.to_string(),
                epics: epics.len(),
                stories,
                total,
                done,
                wip,
                remaining: total - done - wip,
                phase,
            }
        })
        .collect()
}

fn sprint_stories(sprint: &SprintOverview) -> Vec<&StoryOverview> {
    let mut seen = BTreeSet::new();
    let mut keyed = BTreeMap::<String, &StoryOverview>::new();
    for stories in sprint.stories_by_status.values() {
        for story in stories {
            if seen.insert(story.id.clone()) {
                keyed.insert(sprint_story_id(&sprint.sprint_name, &story.id), story);
            }
        }
    }
    keyed.into_values().collect()
}

fn done_points_in_sprint(sprint: &SprintOverview) -> i64 {
    sprint_stories(sprint)
        .into_iter()
        .filter(|story| story_is_done(story))
        .map(story_points)
        .sum()
}

fn planned_points_in_sprint(sprint: &SprintOverview) -> i64 {
    sum_points(sprint_stories(sprint))
}

fn build_sprint_rows(
    sprints: &[SprintOverview],
    forecast: &ReportForecastDto,
    estimates: &EstimateContext,
) -> Vec<ReportSprintProjectionDto> {
    let mut rows = Vec::new();
    let total_delivered: i64 = sprints.iter().map(done_points_in_sprint).sum();
    let mut cumulative_remaining = forecast.remaining_points + total_delivered;
    for sprint in sprints {
        let delivered = done_points_in_sprint(sprint);
        cumulative_remaining -= delivered;
        let status = sprint
            .readme_status
            .clone()
            .unwrap_or_else(|| "planned".to_string());
        let is_past_or_current = status == "closed" || status == "active";
        rows.push(ReportSprintProjectionDto {
            name: sprint.sprint_name.clone(),
            start_date: sprint.start_date.clone(),
            end_date: sprint.end_date.clone(),
            planned_points: Some(planned_points_in_sprint(sprint)),
            delivered_points: is_past_or_current.then_some(delivered),
            rate: (status == "closed").then_some(forecast.throughput.average),
            remaining: is_past_or_current.then_some(cumulative_remaining.max(0)),
            status,
        });
    }

    let Some(last_end) = sprints
        .iter()
        .filter_map(|sprint| parse_date_prefix(&sprint.end_date))
        .max()
    else {
        return rows;
    };
    if estimates.daily_avg <= 0.0 {
        return rows;
    }

    let sprint_days = (forecast.sprint_duration_weeks * 7) as u64;
    let mut projected_remaining = cumulative_remaining as f64;
    let mut sprint_number = sprints.len() + 1;
    let mut projected_index = 1_u64;
    while projected_remaining > 0.0 && projected_index <= 40 {
        let start_date = last_end + Days::new(1 + (projected_index - 1) * sprint_days);
        let end_date = start_date + Days::new(sprint_days.saturating_sub(1));
        let projected_capacity =
            estimates.daily_avg * work_days_inclusive(start_date, end_date) as f64;
        let delivered = projected_capacity.min(projected_remaining);
        projected_remaining = (projected_remaining - delivered).max(0.0);
        rows.push(ReportSprintProjectionDto {
            name: format!("S{:03}.projected", sprint_number),
            start_date: date_string(start_date),
            end_date: date_string(end_date),
            planned_points: Some(projected_capacity.round() as i64),
            delivered_points: Some(delivered.round() as i64),
            rate: Some(round_metric(estimates.daily_avg)),
            remaining: Some(projected_remaining.round() as i64),
            status: format!("projected ({})", estimates.throughput_source),
        });
        projected_index += 1;
        sprint_number += 1;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn story(
        id: &str,
        title: &str,
        status: &str,
        points: &str,
        sprint: Option<&str>,
    ) -> StoryOverview {
        StoryOverview {
            id: id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            epic_id: Some("EP-F1-01".to_string()),
            epic_title: Some("Platform".to_string()),
            assignee: String::new(),
            story_points: points.to_string(),
            sprint: sprint.map(str::to_string),
            relative_path: PathBuf::from(format!("delivery/backlog/{id}.md")),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: None,
            planned_start: None,
            planned_end: None,
        }
    }

    fn sprint(name: &str, status: &str, done_stories: Vec<StoryOverview>) -> SprintOverview {
        let mut stories_by_status = BTreeMap::new();
        stories_by_status.insert("planned".to_string(), Vec::new());
        stories_by_status.insert("todo".to_string(), Vec::new());
        stories_by_status.insert("in-progress".to_string(), Vec::new());
        stories_by_status.insert("ready-for-qa".to_string(), Vec::new());
        stories_by_status.insert("done".to_string(), done_stories);
        stories_by_status.insert("blocked".to_string(), Vec::new());
        SprintOverview {
            sprint_name: name.to_string(),
            headline: name.to_string(),
            sprint_goal: None,
            start_date: "2026-06-01".to_string(),
            end_date: "2026-06-14".to_string(),
            readme_path: PathBuf::from(format!("delivery/sprints/{name}.md")),
            readme_status: Some(status.to_string()),
            wip_limit: None,
            stories_by_status,
            blocked_work: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn epic(id: &str, work_done: Option<&str>) -> EpicOverview {
        EpicOverview {
            id: id.to_string(),
            title: "Platform".to_string(),
            status: "done".to_string(),
            phase: Some("1".to_string()),
            priority: None,
            owner: None,
            milestone: None,
            work_started: None,
            work_done: work_done.map(str::to_string),
            planned_start: None,
            planned_end: None,
            relative_path: PathBuf::from(format!("delivery/backlog/{id}.md")),
        }
    }

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

    #[test]
    fn report_dashboard_builds_web_rows_from_core_inputs() {
        let mut done = story("US-F1-001", "Done story", "done", "5", Some("S000.start"));
        done.work_started = Some("2026-06-01T09:00:00+0200".to_string());
        done.work_done = Some("2026-06-03T12:00:00+0200".to_string());
        let todo = story("US-F1-002", "Todo story", "todo", "8", Some("S001.next"));
        let stories = vec![done.clone(), todo];
        let sprints = vec![sprint("S000.start", "closed", vec![done])];

        let report = ReportDashboardDto::build_with_context(
            &stories,
            &sprints,
            None,
            "2026-06-10T10:00:00+02:00".to_string(),
            chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
        );

        assert_eq!(report.progress.done_points, 5);
        assert_eq!(report.progress.total_points, 13);
        assert_eq!(report.phase_rows[0].phase, "F1");
        assert_eq!(report.phase_rows[0].remaining, 8);
        assert_eq!(report.wbs_rows[0].kind, "phase");
        assert_eq!(report.wbs_rows[1].kind, "epic");
        assert_eq!(report.wbs_rows[2].wbs, "1.1.1");
        assert_eq!(report.wbs_rows[3].status, "TODO");
        assert_eq!(report.sprint_rows[0].delivered_points, Some(5));
        assert!(
            report
                .sprint_rows
                .iter()
                .any(|row| row.status.starts_with("projected"))
        );
    }

    #[test]
    fn report_wbs_includes_precomputed_workbook_rows() {
        let mut done = story("US-F1-001", "Done story", "done", "5", Some("S000.start"));
        done.work_started = Some("2026-06-01T09:00:00+0200".to_string());
        done.work_done = Some("2026-06-03T12:00:00+0200".to_string());
        done.planned_start = Some("2026-06-01".to_string());
        done.planned_end = Some("2026-06-03".to_string());
        let todo = story("US-F1-002", "Todo story", "todo", "8", Some("S001.next"));
        let stories = vec![done.clone(), todo];
        let sprints = vec![sprint("S000.start", "closed", vec![done])];

        let report = ReportWbsDto::build(&stories, &sprints, None);

        assert!(!report.wbs_rows.is_empty());
        assert_eq!(report.wbs_rows[0].kind, "phase");
        assert_eq!(report.wbs_rows[1].kind, "epic");
        assert_eq!(report.wbs_rows[2].kind, "story");
        assert_eq!(
            report.wbs_rows[2].planned_period,
            Some("Q2 2026".to_string())
        );
        assert_eq!(
            report.wbs_rows[2].actual_period,
            Some("Q2 2026".to_string())
        );
        assert_eq!(
            report.wbs_rows[3].notes,
            "Missing planned baseline: start, end"
        );
        assert_eq!(report.phase_rows[0].total, 13);
        assert_eq!(report.sprint_rows[0].name, "S000.start");
        assert!(report.hours_per_point >= 0.0);
    }

    #[test]
    fn workbook_rows_resolve_story_completion_sprints_and_unresolved_notes() {
        let mut done = story(
            "US-F1-001",
            "Done story",
            "done",
            "5",
            Some("S001.foundation"),
        );
        done.work_done = Some("2026-06-03T12:00:00+0200".to_string());
        let mut dropped = story(
            "US-F1-002",
            "Dropped story",
            "dropped",
            "3",
            Some("S002.delivery"),
        );
        dropped.work_done = Some("2026-06-04T12:00:00+0200".to_string());
        let mut missing_sprint = story("US-F1-003", "Missing sprint", "done", "2", None);
        missing_sprint.work_done = Some("2026-06-04T12:00:00+0200".to_string());
        let todo = story(
            "US-F1-004",
            "Todo story",
            "todo",
            "1",
            Some("S001.foundation"),
        );
        let mut first = sprint("S001.foundation", "closed", vec![done.clone()]);
        first.end_date = "2026-06-03".to_string();
        let mut second = sprint("S002.delivery", "closed", vec![dropped.clone()]);
        second.start_date = "2026-06-04".to_string();
        second.end_date = "2026-06-10".to_string();

        let report = ReportWbsDto::build_with_epics(
            &[done, dropped, missing_sprint, todo],
            &[first, second],
            &[],
            None,
        );
        let rows = report.wbs_rows;
        let done_row = rows.iter().find(|row| row.id == "US-F1-001").unwrap();
        let dropped_row = rows.iter().find(|row| row.id == "US-F1-002").unwrap();
        let missing_row = rows.iter().find(|row| row.id == "US-F1-003").unwrap();
        let todo_row = rows.iter().find(|row| row.id == "US-F1-004").unwrap();

        assert_eq!(
            done_row.completed_in_sprint.as_deref(),
            Some("S001.foundation")
        );
        assert_eq!(
            dropped_row.completed_in_sprint.as_deref(),
            Some("S002.delivery")
        );
        assert!(missing_row.completed_in_sprint.is_none());
        assert!(
            missing_row
                .notes
                .contains("terminal story has no retained sprint")
        );
        assert!(todo_row.completed_in_sprint.is_none());
        assert!(
            rows.iter()
                .find(|row| row.kind == "phase")
                .unwrap()
                .completed_in_sprint
                .is_none()
        );
        assert!(
            rows.iter()
                .find(|row| row.kind == "epic")
                .unwrap()
                .completed_in_sprint
                .is_none()
        );
    }

    #[test]
    fn workbook_rows_resolve_epic_completion_on_inclusive_sprint_boundaries() {
        let mut boundary_story = story("US-F1-001", "Boundary story", "done", "1", None);
        boundary_story.work_done = Some("2026-06-14T12:00:00+0200".to_string());
        let mut sprint = sprint("S001.foundation", "closed", vec![]);
        sprint.start_date = "2026-06-01".to_string();
        sprint.end_date = "2026-06-14".to_string();

        let report = ReportWbsDto::build_with_epics(
            &[boundary_story],
            &[sprint],
            &[epic("EP-F1-01", Some("2026-06-14T23:59:59+0200"))],
            None,
        );
        let epic_row = report
            .wbs_rows
            .iter()
            .find(|row| row.kind == "epic")
            .unwrap();
        assert_eq!(
            epic_row.completed_in_sprint.as_deref(),
            Some("S001.foundation")
        );
    }

    #[test]
    fn workbook_rows_leave_unresolved_epic_completion_blank_with_note() {
        let story = story("US-F1-001", "Done story", "done", "1", None);
        let mut sprint = sprint("S001.foundation", "closed", vec![]);
        sprint.start_date = "2026-06-01".to_string();
        sprint.end_date = "2026-06-14".to_string();

        let report = ReportWbsDto::build_with_epics(
            &[story],
            &[sprint],
            &[epic("EP-F1-01", Some("2026-07-01T12:00:00+0200"))],
            None,
        );
        let epic_row = report
            .wbs_rows
            .iter()
            .find(|row| row.kind == "epic")
            .unwrap();
        assert!(epic_row.completed_in_sprint.is_none());
        assert!(
            epic_row
                .notes
                .contains("Unable to resolve completion sprint")
        );
    }
}
