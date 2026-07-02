import { describe, expect, it } from "vitest";
import { computeEstimates, roundMetric, sumPoints } from "./estimates.js";
import { story, repository, metrics } from "./fixtures.js";

describe("roundMetric", () => {
  it("rounds to one decimal place", () => {
    expect(roundMetric(2.666)).toBe(2.7);
    expect(roundMetric(2.64)).toBe(2.6);
    expect(roundMetric(7)).toBe(7);
  });
});

describe("sumPoints", () => {
  it("sums storyPoints across stories", () => {
    const stories = [
      story({ id: "US-F1-001", title: "a", status: "done", storyPoints: 5 }),
      story({ id: "US-F1-002", title: "b", status: "todo", storyPoints: 3 }),
    ];
    expect(sumPoints(stories)).toBe(8);
  });

  it("treats null storyPoints as 0", () => {
    const stories = [story({ id: "US-F1-001", title: "a", status: "todo", storyPoints: null })];
    expect(sumPoints(stories)).toBe(0);
  });

  it("excludes dropped storyPoints from totals", () => {
    const stories = [
      story({ id: "US-F1-001", title: "a", status: "done", storyPoints: 5 }),
      story({ id: "US-F1-002", title: "b", status: "dropped", storyPoints: 3 }),
    ];
    expect(sumPoints(stories)).toBe(5);
  });
});

describe("computeEstimates", () => {
  it("returns null estimates when throughput is 0", () => {
    const repo = repository();
    const m = metrics({
      forecast: {
        ...metrics().forecast,
        throughput: { samples: [], average: 0, median: 0, observedDayCount: 0 },
      },
    });
    // Remove past sprints so fallback is also 0
    const emptyRepo = { ...repo, sprints: [] };
    const result = computeEstimates(repo.stories, m, emptyRepo);
    expect(result.dailyAvg).toBe(0);
    expect(result.hoursPerPoint).toBe(0);
    for (const estimate of result.estimates.values()) {
      expect(estimate.estHours).toBeNull();
      expect(estimate.estStart).toBeNull();
      expect(estimate.estEnd).toBeNull();
    }
  });

  it("assigns sequential start dates for todo stories", () => {
    const s1 = story({ id: "US-F1-002", title: "Todo 1", status: "todo", storyPoints: 2 });
    const s2 = story({ id: "US-F1-003", title: "Todo 2", status: "todo", storyPoints: 3 });
    const repo = repository({ stories: [s1, s2], sprints: [] });
    const m = metrics();
    const result = computeEstimates([s1, s2], m, repo);

    const est1 = result.estimates.get("US-F1-002")!;
    const est2 = result.estimates.get("US-F1-003")!;
    expect(est1.estStart).not.toBeNull();
    expect(est2.estStart).not.toBeNull();
    // second story starts after first story ends
    expect(est2.estStart! >= est1.estEnd!).toBe(true);
  });

  it("computes hoursPerPoint = 7 / dailyAvg", () => {
    const repo = repository();
    const m = metrics();
    const result = computeEstimates(repo.stories, m, repo);
    const expectedHoursPerPoint = 7 / m.forecast.throughput.average;
    expect(result.hoursPerPoint).toBeCloseTo(expectedHoursPerPoint, 5);
  });

  it("sets estStart from workStarted for in-progress stories", () => {
    const inProgress = story({
      id: "US-F1-010",
      title: "In Progress",
      status: "in-progress",
      storyPoints: 5,
      workStarted: "2026-06-05T09:00:00+0200",
    });
    const repo = repository({ stories: [inProgress], sprints: [] });
    const m = metrics();
    const result = computeEstimates([inProgress], m, repo);
    const est = result.estimates.get("US-F1-010")!;
    expect(est.estStart).toBe("2026-06-05");
    expect(est.estEnd).not.toBeNull();
  });

  it("populates done stories with actual work dates", () => {
    const done = story({
      id: "US-F1-001",
      title: "Done story",
      status: "done",
      storyPoints: 5,
      workStarted: "2026-06-01T09:00:00+0200",
      workDone: "2026-06-03T12:00:00+0200",
    });
    const repo = repository({ stories: [done], sprints: [] });
    const m = metrics();
    const result = computeEstimates([done], m, repo);
    const est = result.estimates.get("US-F1-001")!;
    expect(est.estStart).toBe("2026-06-01");
    expect(est.estEnd).toBe("2026-06-03");
  });
});
