import type { ReportDashboard } from "@shared/generated/api.js";

export interface Estimate {
  estHours: number | null;
  estStart: string | null;
  estEnd: string | null;
}

export function roundMetric(value: number): number {
  return Number(value.toFixed(1));
}

export function estimatesByStory(report: ReportDashboard): Map<string, Estimate> {
  return new Map(report.estimates.map((estimate) => [estimate.storyId, {
    estHours: estimate.estHours,
    estStart: estimate.estStart,
    estEnd: estimate.estEnd,
  }]));
}
