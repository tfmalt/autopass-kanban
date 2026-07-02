import { describe, expect, it } from "vitest";
import { buildSprintRows, phaseRows } from "./sprints.js";
import { story, repository, metrics } from "./fixtures.js";

describe("buildSprintRows", () => {
  it("marks closed sprints with delivered points", () => {
    const repo = repository();
    const m = metrics();
    const rows = buildSprintRows(repo, m, m.forecast.throughput.average, "daily throughput");

    const closedRow = rows.find((r) => r.name === "S000.start")!;
    expect(closedRow.status).toBe("closed");
    expect(closedRow.deliveredPoints).toBe(5);
  });

  it("leaves deliveredPoints null for planned sprints", () => {
    const done = story({ id: "US-F1-001", title: "Done", status: "done", storyPoints: 5 });
    const planned = story({ id: "US-F1-002", title: "Planned", status: "todo", storyPoints: 3 });
    const repo = {
      ...repository(),
      sprints: [
        {
          name: "S000.start",
          id: "S000",
          headline: "start",
          goal: null,
          startDate: "2026-06-01",
          endDate: "2026-06-14",
          status: "planned" as const,
          wipLimit: null,
          storiesByStatus: { planned: [planned], todo: [], "in-progress": [], "ready-for-qa": [], done: [done], blocked: [] },
        },
      ],
    };
    const m = metrics();
    const rows = buildSprintRows(repo, m, m.forecast.throughput.average, "daily throughput");

    const row = rows.find((r) => r.name === "S000.start")!;
    expect(row.deliveredPoints).toBeNull();
    expect(row.remaining).toBeNull();
  });

  it("appends projected sprints until remaining reaches 0", () => {
    const repo = repository();
    const m = metrics();
    const rows = buildSprintRows(repo, m, m.forecast.throughput.average, "daily throughput");

    const projectedRows = rows.filter((r) => r.status.startsWith("projected"));
    expect(projectedRows.length).toBeGreaterThan(0);

    const lastProjected = projectedRows.at(-1)!;
    expect(lastProjected.remaining).toBe(0);
  });

  it("caps projected sprints at 40", () => {
    // Use a very small throughput so many sprints are needed
    const repo = repository({
      stories: [story({ id: "US-F1-001", title: "Huge", status: "todo", storyPoints: 9999 })],
      sprints: [
        {
          name: "S000.start",
          id: "S000",
          headline: "start",
          goal: null,
          startDate: "2026-06-01",
          endDate: "2026-06-14",
          status: "closed",
          wipLimit: null,
          storiesByStatus: { planned: [], todo: [], "in-progress": [], "ready-for-qa": [], done: [], blocked: [] },
        },
      ],
      progress: { donePoints: 0, totalPoints: 9999, doneStories: 0, totalStories: 1, phases: [] },
    });
    const m = metrics({
      forecast: {
        ...metrics().forecast,
        remainingPoints: 9999,
        throughput: { samples: [0.01], average: 0.01, median: 0.01, observedDayCount: 1 },
      },
    });
    const rows = buildSprintRows(repo, m, 0.01, "daily throughput");
    const projectedRows = rows.filter((r) => r.status.startsWith("projected"));
    expect(projectedRows.length).toBeLessThanOrEqual(40);
  });

  it("returns no projected rows when dailyAvg is 0", () => {
    const repo = repository();
    const m = metrics();
    const rows = buildSprintRows(repo, m, 0, "no throughput data");

    const projectedRows = rows.filter((r) => r.status.startsWith("projected"));
    expect(projectedRows).toHaveLength(0);
  });
});

describe("phaseRows", () => {
  it("returns one row per phase with correct counts", () => {
    const repo = repository();
    const rows = phaseRows(repo);
    expect(rows).toHaveLength(1);
    expect(rows[0]!.phase).toBe("F1");
    expect(rows[0]!.total).toBe(13);
    expect(rows[0]!.done).toBe(5);
  });

  it("counts WIP separately from done and remaining", () => {
    const done = story({ id: "US-F1-001", title: "Done", status: "done", storyPoints: 5 });
    const wip = story({ id: "US-F1-002", title: "WIP", status: "in-progress", storyPoints: 3 });
    const todo = story({ id: "US-F1-003", title: "Todo", status: "todo", storyPoints: 4 });
    const repo = {
      ...repository(),
      stories: [done, wip, todo],
      progress: {
        donePoints: 5,
        totalPoints: 12,
        doneStories: 1,
        totalStories: 3,
        phases: [{ phase: "F1", donePoints: 5, totalPoints: 12, doneStories: 1, totalStories: 3 }],
      },
    };
    const rows = phaseRows(repo);
    expect(rows[0]!.wip).toBe(3);
    expect(rows[0]!.remaining).toBe(12 - 5 - 3);
  });

  it("excludes dropped points from phase totals and done totals", () => {
    const done = story({ id: "US-F1-001", title: "Done", status: "done", storyPoints: 5 });
    const dropped = story({ id: "US-F1-002", title: "Dropped", status: "dropped", storyPoints: 3 });
    const todo = story({ id: "US-F1-003", title: "Todo", status: "todo", storyPoints: 4 });
    const repo = {
      ...repository(),
      stories: [done, dropped, todo],
      progress: {
        donePoints: 5,
        totalPoints: 9,
        doneStories: 1,
        totalStories: 2,
        phases: [{ phase: "F1", donePoints: 5, totalPoints: 9, doneStories: 1, totalStories: 2 }],
      },
    };
    const rows = phaseRows(repo);
    expect(rows[0]!.done).toBe(5);
    expect(rows[0]!.total).toBe(9);
    expect(rows[0]!.remaining).toBe(4);
  });
});
