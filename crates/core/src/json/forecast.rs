use serde::Serialize;

use crate::{SprintOverview, StoryOverview, StoryStatus, WorkingCalendar};

use super::parse_points;

/// Daily throughput distribution used by the canonical forecast model.
#[derive(Debug, Clone, Serialize)]
pub struct ForecastThroughputDto {
    pub samples: Vec<i64>,
    pub average: f64,
    pub median: f64,
    pub observed_day_count: usize,
}

/// Probabilistic completion bands from deterministic Monte Carlo simulation.
#[derive(Debug, Clone, Serialize)]
pub struct ForecastCompletionDto {
    pub p50_days: Option<u32>,
    pub p80_days: Option<u32>,
    pub p90_days: Option<u32>,
    pub p50_date: Option<String>,
    pub p80_date: Option<String>,
    pub p90_date: Option<String>,
}

/// Canonical planning forecast shared by CLI, web, and generated reports.
#[derive(Debug, Clone, Serialize)]
pub struct ReportForecastDto {
    pub generated_at: String,
    pub remaining_points: i64,
    pub sprint_duration_weeks: u32,
    pub projection_start_date: String,
    pub throughput: ForecastThroughputDto,
    pub completion: ForecastCompletionDto,
    pub confidence: String,
}

pub(crate) struct ForecastInputs {
    pub(crate) generated_at: String,
    pub(crate) remaining_points: i64,
    pub(crate) sprint_duration_weeks: u32,
    pub(crate) projection_start_date: chrono::NaiveDate,
    pub(crate) throughput_samples: Vec<i64>,
    pub(crate) calendar: WorkingCalendar,
}

fn average(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<i64>() as f64 / values.len() as f64
    }
}

fn median(values: &[i64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
    } else {
        sorted[mid] as f64
    }
}

fn next_random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed
}

fn random_index(seed: &mut u64, len: usize) -> usize {
    debug_assert!(len > 0);
    ((next_random(seed) >> 32) as usize) % len
}

fn percentile(sorted_values: &[u32], percentile: f64) -> Option<u32> {
    if sorted_values.is_empty() {
        return None;
    }
    let index = ((sorted_values.len() as f64 * percentile).ceil() as usize).saturating_sub(1);
    sorted_values.get(index).copied()
}

fn completion_date(
    start: chrono::NaiveDate,
    days: Option<u32>,
    calendar: &WorkingCalendar,
) -> Option<String> {
    Some(
        calendar
            .add_capacity_days(start, f64::from(days?))
            .format("%Y-%m-%d")
            .to_string(),
    )
}

fn parse_frontmatter_date(value: &str) -> Option<chrono::NaiveDate> {
    let date_part = value.trim().get(..10)?;
    chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

pub(crate) fn daily_throughput_samples(
    stories: &[StoryOverview],
    today: chrono::NaiveDate,
    calendar: &WorkingCalendar,
) -> Vec<i64> {
    let mut points_by_day: std::collections::BTreeMap<chrono::NaiveDate, i64> =
        std::collections::BTreeMap::new();
    for story in stories {
        if StoryStatus::parse(&story.status) != Some(StoryStatus::Done) {
            continue;
        }
        let Some(work_done) = story.work_done.as_deref().and_then(parse_frontmatter_date) else {
            continue;
        };
        let points = parse_points(&story.story_points).unwrap_or(0);
        if points <= 0 {
            continue;
        }
        *points_by_day.entry(work_done).or_default() += points;
    }

    let Some(first_day) = points_by_day.keys().next().copied() else {
        return Vec::new();
    };

    let end_day = today.max(
        points_by_day
            .keys()
            .next_back()
            .copied()
            .unwrap_or(first_day),
    );
    let mut samples = Vec::new();
    let mut day = first_day;
    while day <= end_day {
        let completed = *points_by_day.get(&day).unwrap_or(&0);
        let capacity = calendar.day_capacity(day);
        if capacity > 0.0 || completed > 0 {
            let normalized = if capacity > 0.0 {
                (completed as f64 / capacity).round() as i64
            } else {
                completed
            };
            samples.push(normalized);
        }
        day += chrono::Duration::days(1);
    }
    samples
}

fn simulate_completion_days(remaining_points: i64, samples: &[i64]) -> Vec<u32> {
    if remaining_points <= 0 {
        return vec![0];
    }
    if samples.is_empty() || samples.iter().all(|sample| *sample <= 0) {
        return Vec::new();
    }

    const ITERATIONS: usize = 10_000;
    const MAX_DAYS: u32 = 10_000;
    let mut seed = 0xA17C_0DE5_u64;
    let mut results = Vec::with_capacity(ITERATIONS);

    for _ in 0..ITERATIONS {
        let mut remaining = remaining_points;
        let mut days = 0_u32;
        while remaining > 0 && days < MAX_DAYS {
            let idx = random_index(&mut seed, samples.len());
            remaining -= samples[idx].max(0);
            days += 1;
        }
        if remaining <= 0 {
            results.push(days);
        }
    }

    results.sort_unstable();
    results
}

impl ReportForecastDto {
    pub(crate) fn from_inputs(inputs: ForecastInputs) -> Self {
        let samples = inputs.throughput_samples;
        let observed_day_count = samples.len();
        let simulations = simulate_completion_days(inputs.remaining_points, &samples);
        let p50 = percentile(&simulations, 0.50);
        let p80 = percentile(&simulations, 0.80);
        let p90 = percentile(&simulations, 0.90);
        let confidence = if observed_day_count == 0 || simulations.is_empty() {
            "none"
        } else if observed_day_count < 5 {
            "low"
        } else if observed_day_count < 10 {
            "medium"
        } else {
            "high"
        };

        Self {
            generated_at: inputs.generated_at,
            remaining_points: inputs.remaining_points,
            sprint_duration_weeks: inputs.sprint_duration_weeks,
            projection_start_date: inputs.projection_start_date.format("%Y-%m-%d").to_string(),
            throughput: ForecastThroughputDto {
                average: average(&samples),
                median: median(&samples),
                observed_day_count,
                samples,
            },
            completion: ForecastCompletionDto {
                p50_days: p50,
                p80_days: p80,
                p90_days: p90,
                p50_date: completion_date(inputs.projection_start_date, p50, &inputs.calendar),
                p80_date: completion_date(inputs.projection_start_date, p80, &inputs.calendar),
                p90_date: completion_date(inputs.projection_start_date, p90, &inputs.calendar),
            },
            confidence: confidence.to_string(),
        }
    }

    pub fn build(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        _current_sprint_name: Option<&str>,
    ) -> Self {
        Self::build_with_calendar(
            stories,
            sprints,
            _current_sprint_name,
            WorkingCalendar::empty(),
        )
    }

    pub fn build_with_calendar(
        stories: &[StoryOverview],
        sprints: &[SprintOverview],
        _current_sprint_name: Option<&str>,
        calendar: WorkingCalendar,
    ) -> Self {
        let generated_at = chrono::Local::now().to_rfc3339();
        let today = chrono::Local::now().date_naive();
        let remaining_points: i64 = stories
            .iter()
            .filter(|s| {
                StoryStatus::parse_counts_toward_scope(&s.status)
                    && !StoryStatus::parse_is_terminal(&s.status)
            })
            .map(|s| parse_points(&s.story_points).unwrap_or(0))
            .sum();
        let sprint_duration_weeks = sprints
            .first()
            .and_then(|s| {
                let start = chrono::NaiveDate::parse_from_str(&s.start_date, "%Y-%m-%d").ok()?;
                let end = chrono::NaiveDate::parse_from_str(&s.end_date, "%Y-%m-%d").ok()?;
                Some(((end - start).num_days() as f64 / 7.0).round() as u32)
            })
            .unwrap_or(2)
            .max(1);

        Self::from_inputs(ForecastInputs {
            generated_at,
            remaining_points,
            sprint_duration_weeks,
            projection_start_date: today,
            throughput_samples: daily_throughput_samples(stories, today, &calendar),
            calendar,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn canonical_forecast_serializes_probability_bands() {
        let forecast = ReportForecastDto::from_inputs(ForecastInputs {
            generated_at: "2026-06-09T10:00:00+02:00".to_string(),
            remaining_points: 20,
            sprint_duration_weeks: 2,
            projection_start_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 9).unwrap(),
            throughput_samples: vec![5, 10, 15],
            calendar: WorkingCalendar::empty(),
        });

        let json = serde_json::to_value(&forecast).expect("serialization should succeed");
        assert_eq!(json["remaining_points"], 20);
        assert_eq!(json["throughput"]["average"], 10.0);
        assert_eq!(json["throughput"]["median"], 10.0);
        assert_eq!(json["confidence"], "low");
        assert!(json["completion"]["p50_days"].as_u64().unwrap() >= 2);
        assert!(json["completion"]["p90_date"].is_string());
    }

    #[test]
    fn canonical_forecast_has_no_completion_without_throughput() {
        let forecast = ReportForecastDto::from_inputs(ForecastInputs {
            generated_at: "2026-06-09T10:00:00+02:00".to_string(),
            remaining_points: 20,
            sprint_duration_weeks: 2,
            projection_start_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 9).unwrap(),
            throughput_samples: vec![],
            calendar: WorkingCalendar::empty(),
        });

        assert_eq!(forecast.confidence, "none");
        assert_eq!(forecast.completion.p80_date, None);
        assert_eq!(forecast.throughput.average, 0.0);
    }

    #[test]
    fn daily_throughput_samples_group_done_points_and_include_zero_weekdays() {
        fn story(id: &str, status: &str, points: &str, work_done: Option<&str>) -> StoryOverview {
            StoryOverview {
                id: id.to_string(),
                title: id.to_string(),
                status: status.to_string(),
                epic_id: None,
                epic_title: None,
                assignee: String::new(),
                story_points: points.to_string(),
                sprint: Some("S001".to_string()),
                relative_path: PathBuf::from(format!("delivery/backlog/{id}.md")),
                task_summary: None,
                task_count: 0,
                work_started: None,
                work_done: work_done.map(str::to_string),
                planned_start: None,
                planned_end: None,
            }
        }

        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let samples = daily_throughput_samples(
            &[
                story("US-F1-001", "done", "5", Some("2026-06-08T12:00:00+0200")),
                story("US-F1-002", "done", "3", Some("2026-06-08T13:00:00+0200")),
                story("US-F1-003", "todo", "13", None),
                story("US-F1-004", "done", "2", Some("2026-06-10T09:00:00+0200")),
            ],
            today,
            &WorkingCalendar::empty(),
        );

        assert_eq!(samples, vec![8, 0, 2]);
    }

    #[test]
    fn monte_carlo_forecast_spreads_percentiles_for_power_of_two_sample_sets() {
        let forecast = ReportForecastDto::from_inputs(ForecastInputs {
            generated_at: "2026-06-17T10:00:00+02:00".to_string(),
            remaining_points: 906,
            sprint_duration_weeks: 2,
            projection_start_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 17).unwrap(),
            throughput_samples: vec![23, 6, 3, 0, 0, 5, 0, 5, 5, 17, 0, 20, 0, 0, 0, 0],
            calendar: WorkingCalendar::empty(),
        });

        assert!(forecast.completion.p50_days.is_some());
        assert!(forecast.completion.p80_days.is_some());
        assert!(forecast.completion.p90_days.is_some());

        let p50 = forecast.completion.p50_days.unwrap();
        let p80 = forecast.completion.p80_days.unwrap();
        let p90 = forecast.completion.p90_days.unwrap();

        assert!(p50 < p80, "expected P50 < P80, got {p50} and {p80}");
        assert!(p80 <= p90, "expected P80 <= P90, got {p80} and {p90}");
    }

    #[test]
    fn throughput_excludes_hiatus_and_normalizes_partial_capacity() {
        let team = vec![
            crate::TeamMemberConfig {
                name: "Ada".to_string(),
                email: "ada@example.com".to_string(),
                avatar_url: None,
                avatar_path: None,
            },
            crate::TeamMemberConfig {
                name: "Grace".to_string(),
                email: "grace@example.com".to_string(),
                avatar_url: None,
                avatar_path: None,
            },
        ];
        let calendar = crate::parse_availability_markdown(
            "| ID | Type | Who | Start | End | Availability | Note |\n|---|---|---|---|---|---:|---|\n| AV-001 | hiatus | * | 2026-06-09 | 2026-06-09 | 0% | Pause |\n| AV-002 | vacation | ada@example.com | 2026-06-10 | 2026-06-10 | 0% | Away |",
            &team,
        )
        .unwrap();
        let stories = vec![StoryOverview {
            id: "US-001".to_string(),
            title: "Done".to_string(),
            status: "done".to_string(),
            epic_id: None,
            epic_title: None,
            assignee: String::new(),
            story_points: "2".to_string(),
            sprint: None,
            relative_path: PathBuf::from("US-001.md"),
            task_summary: None,
            task_count: 0,
            work_started: None,
            work_done: Some("2026-06-10T12:00:00+02:00".to_string()),
            planned_start: None,
            planned_end: None,
        }];

        assert_eq!(
            daily_throughput_samples(
                &stories,
                chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
                &calendar,
            ),
            vec![4]
        );
    }
}
