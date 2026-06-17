import type { DashboardMetrics, RepositorySnapshot } from "@shared/types.js";
import { normalizeStatus } from "@shared/types.js";
import { addDays, workDaysInclusive } from "@shared/dates.js";
import { PHASE_META } from "./meta.js";
import { roundMetric, sumPoints } from "./estimates.js";

export interface SprintProjection {
  name: string;
  startDate: string;
  endDate: string;
  plannedPoints: number | null;
  deliveredPoints: number | null;
  rate: number | null;
  remaining: number | null;
  status: string;
}

export function buildSprintRows(repo: RepositorySnapshot, metrics: DashboardMetrics, dailyAvg: number, source: string): SprintProjection[] {
  const rows: SprintProjection[] = [];
  const totalDelivered = repo.sprints.reduce((sum, sprint) => sum + sumPoints(sprint.storiesByStatus.done), 0);
  let cumulativeRemaining = metrics.forecast.remainingPoints + totalDelivered;
  for (const sprint of repo.sprints) {
    const delivered = sumPoints(sprint.storiesByStatus.done);
    cumulativeRemaining -= delivered;
    const isPastOrCurrent = sprint.status === "closed" || sprint.status === "active";
    rows.push({
      name: sprint.name,
      startDate: sprint.startDate ?? "",
      endDate: sprint.endDate ?? "",
      plannedPoints: sumPoints(Object.values(sprint.storiesByStatus).flat()),
      deliveredPoints: isPastOrCurrent ? delivered : null,
      rate: sprint.status === "closed" ? metrics.forecast.throughput.average : null,
      remaining: isPastOrCurrent ? Math.max(0, cumulativeRemaining) : null,
      status: sprint.status ?? "planned",
    });
  }

  const lastEnd = repo.sprints.map((sprint) => sprint.endDate).filter((value): value is string => Boolean(value)).sort().at(-1);
  if (!lastEnd || dailyAvg <= 0) return rows;
  let projectedRemaining = cumulativeRemaining;
  const sprintDays = metrics.forecast.sprintDurationWeeks * 7;
  let sprintNumber = repo.sprints.length + 1;
  let projectedIndex = 1;
  while (projectedRemaining > 0 && projectedIndex <= 40) {
    const startDate = addDays(lastEnd, 1 + (projectedIndex - 1) * sprintDays);
    const endDate = addDays(startDate, sprintDays - 1);
    const projectedCapacity = dailyAvg * workDaysInclusive(startDate, endDate);
    const delivered = Math.min(projectedCapacity, projectedRemaining);
    projectedRemaining = Math.max(0, projectedRemaining - delivered);
    rows.push({
      name: `S${String(sprintNumber).padStart(3, "0")}.projected`,
      startDate,
      endDate,
      plannedPoints: Math.round(projectedCapacity),
      deliveredPoints: Math.round(delivered),
      rate: roundMetric(dailyAvg),
      remaining: Math.round(projectedRemaining),
      status: `projected (${source})`,
    });
    projectedIndex += 1;
    sprintNumber += 1;
  }
  return rows;
}

export function phaseRows(repo: RepositorySnapshot): Array<{ phase: string; title: string; period: string; milestone: string; epics: number; stories: number; total: number; done: number; wip: number; remaining: number }> {
  return repo.progress.phases.map((phase) => {
    const meta = PHASE_META[phase.phase] ?? { title: phase.phase, period: "", milestone: "", priority: "" };
    const stories = repo.stories.filter((story) => story.phase === phase.phase);
    const epics = new Set(stories.map((story) => story.epic ?? "?")).size;
    const wip = stories.filter((story) => ["in-progress", "ready-for-qa"].includes(normalizeStatus(story.status))).reduce((sum, story) => sum + (story.storyPoints ?? 0), 0);
    return {
      phase: phase.phase,
      title: meta.title,
      period: meta.period,
      milestone: meta.milestone,
      epics,
      stories: phase.totalStories,
      total: phase.totalPoints,
      done: phase.donePoints,
      wip,
      remaining: phase.totalPoints - phase.donePoints - wip,
    };
  });
}
