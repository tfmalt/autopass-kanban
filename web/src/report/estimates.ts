import type { DashboardMetrics, RepositorySnapshot, Story } from "@shared/types.js";
import { normalizeStatus } from "@shared/types.js";
import { addWorkingDays, parseDate } from "@shared/dates.js";

export interface Estimate {
  estHours: number | null;
  estStart: string | null;
  estEnd: string | null;
}

export function roundMetric(value: number): number {
  return Number(value.toFixed(1));
}

export function sumPoints(stories: Story[]): number {
  return stories.reduce((sum, story) => sum + (normalizeStatus(story.status) === "dropped" ? 0 : (story.storyPoints ?? 0)), 0);
}

export function throughputSource(metrics: DashboardMetrics, repo: RepositorySnapshot): { dailyAvg: number; label: string } {
  const dailyAvg = metrics.forecast.throughput.average;
  const observed = metrics.forecast.throughput.observedDayCount;
  if (dailyAvg > 0) return { dailyAvg, label: `daily throughput over ${observed} observed workdays` };

  const past = repo.sprints.filter((sprint) => sprint.endDate && sprint.endDate < metrics.forecast.projectionStartDate);
  const delivered = past.map((sprint) => sumPoints(sprint.storiesByStatus.done));
  const avgSprint = delivered.length > 0 ? delivered.reduce((sum, value) => sum + value, 0) / delivered.length : 0;
  const dailyFallback = avgSprint / Math.max(1, metrics.forecast.sprintDurationWeeks * 5);
  return dailyFallback > 0
    ? { dailyAvg: dailyFallback, label: "sprint velocity fallback" }
    : { dailyAvg: 0, label: "no throughput data" };
}

export function sortStoriesForEstimates(stories: Story[]): Story[] {
  const statusOrder: Record<string, number> = {
    "in-progress": 0,
    "ready-for-qa": 1,
    todo: 2,
    planned: 3,
    ready: 4,
    draft: 5,
    blocked: 6,
  };
  return [...stories].sort((a, b) =>
    (statusOrder[normalizeStatus(a.status)] ?? 9) - (statusOrder[normalizeStatus(b.status)] ?? 9)
    || (a.phase ?? "").localeCompare(b.phase ?? "")
    || (a.epic ?? "").localeCompare(b.epic ?? "")
    || a.id.localeCompare(b.id),
  );
}

export function computeEstimates(stories: Story[], metrics: DashboardMetrics, repo: RepositorySnapshot): { estimates: Map<string, Estimate>; hoursPerPoint: number; source: string; dailyAvg: number } {
  const { dailyAvg, label } = throughputSource(metrics, repo);
  const estimates = new Map<string, Estimate>();
  if (dailyAvg <= 0) {
    for (const story of stories) estimates.set(story.id, { estHours: null, estStart: null, estEnd: null });
    return { estimates, hoursPerPoint: 0, source: label, dailyAvg };
  }

  const today = metrics.forecast.projectionStartDate;
  const hoursPerPoint = 7 / dailyAvg;
  const daysPerPoint = 1 / dailyAvg;
  let cumulativeDays = 0;

  for (const story of sortStoriesForEstimates(stories).filter((s) => !["done", "dropped"].includes(normalizeStatus(s.status)))) {
    const points = story.storyPoints ?? 0;
    if (points <= 0) {
      estimates.set(story.id, { estHours: null, estStart: null, estEnd: null });
      continue;
    }

    const estHours = roundMetric(points * hoursPerPoint);
    const duration = points * daysPerPoint;
    const workStarted = parseDate(story.workStarted);
    if (workStarted) {
      estimates.set(story.id, { estHours, estStart: workStarted, estEnd: addWorkingDays(today, duration) });
    } else {
      estimates.set(story.id, {
        estHours,
        estStart: addWorkingDays(today, cumulativeDays),
        estEnd: addWorkingDays(today, cumulativeDays + duration),
      });
      cumulativeDays += duration;
    }
  }

  for (const story of stories) {
    if (!estimates.has(story.id)) {
      estimates.set(story.id, { estHours: null, estStart: parseDate(story.workStarted), estEnd: parseDate(story.workDone) });
    }
  }

  return { estimates, hoursPerPoint, source: label, dailyAvg };
}

export function groupDates(stories: Story[], estimates: Map<string, Estimate>): { start: string | null; end: string | null } {
  const starts: string[] = [];
  const ends: string[] = [];
  for (const story of stories) {
    const status = normalizeStatus(story.status);
    const started = parseDate(story.workStarted);
    const done = parseDate(story.workDone);
    const estimate = estimates.get(story.id);
    if (status === "done" || status === "dropped") {
      if (started) starts.push(started);
      if (done) ends.push(done);
    } else if (["in-progress", "ready-for-qa"].includes(status)) {
      if (started) starts.push(started);
      if (estimate?.estEnd) ends.push(estimate.estEnd);
    } else {
      if (estimate?.estStart) starts.push(estimate.estStart);
      if (estimate?.estEnd) ends.push(estimate.estEnd);
    }
  }
  return {
    start: starts.length > 0 ? starts.sort()[0]! : null,
    end: ends.length > 0 ? ends.sort().at(-1)! : null,
  };
}
