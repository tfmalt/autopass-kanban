import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { DashboardMetrics, RepositorySnapshot, Story } from "@shared/types.js";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";
import { ReportView } from "./ReportView.js";

function story(input: Partial<Story> & Pick<Story, "id" | "title" | "status" | "storyPoints">): Story {
  const {
    id,
    title,
    status,
    storyPoints,
    phase,
    epic,
    sprint,
    priority,
    assignee,
    assignees,
    workStarted,
    workDone,
    activated,
    created,
    updated,
    relativePath,
    tasks,
    taskSummary,
    frontmatter,
  } = input;
  return {
    id,
    title,
    status,
    storyPoints,
    phase: phase ?? "F1",
    epic: epic ?? "EP-F1-01",
    sprint: sprint ?? null,
    priority: priority ?? null,
    assignee: assignee ?? null,
    assignees: assignees ?? [],
    workStarted: workStarted ?? null,
    workDone: workDone ?? null,
    activated: activated ?? null,
    created: created ?? null,
    updated: updated ?? null,
    relativePath: relativePath ?? "x",
    tasks: tasks ?? [],
    taskSummary: taskSummary ?? { todo: 0, inProgress: 0, readyForQa: 0, done: 0, blocked: 0, total: 0 },
    frontmatter: frontmatter ?? {},
  };
}

function repository(): RepositorySnapshot {
  const done = story({
    id: "US-F1-001",
    title: "Done story",
    status: "done",
    storyPoints: 5,
    sprint: "S000.start",
    workStarted: "2026-06-01T09:00:00+0200",
    workDone: "2026-06-03T12:00:00+0200",
  });
  const todo = story({
    id: "US-F1-002",
    title: "Todo story",
    status: "todo",
    storyPoints: 8,
    sprint: "S001.next",
    assignee: "Test User <test@example.com>",
  });
  return {
    stories: [done, todo],
    epics: [{ id: "EP-F1-01", title: "Platform", phase: "F1", priority: null, stories: [done, todo] }],
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
        storiesByStatus: { planned: [], todo: [], "in-progress": [], "ready-for-qa": [], done: [done], blocked: [] },
      },
    ],
    progress: { donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2, phases: [{ phase: "F1", donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2 }] },
  };
}

function metrics(): DashboardMetrics {
  return {
    burnup: [],
    burndown: [],
    leadTime: [],
    velocity: [{ sprint: "S000.start", points: 5, forecast: false }],
    forecast: {
      generatedAt: "2026-06-10T10:00:00+0200",
      remainingPoints: 8,
      sprintDurationWeeks: 2,
      projectionStartDate: "2026-06-10",
      throughput: { samples: [5, 0, 3], average: 2.67, median: 3, observedDayCount: 3 },
      completion: {
        p50Days: 3,
        p80Days: 4,
        p90Days: 5,
        p50Date: "2026-06-15",
        p80Date: "2026-06-16",
        p90Date: "2026-06-17",
      },
      confidence: "low",
    },
    progress: repository().progress,
  };
}

function renderWithClient(ui: ReactNode) {
  const qc = new QueryClient();
  qc.setQueryData(["repository"], repository());
  qc.setQueryData(["metrics"], metrics());
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

describe("ReportView", () => {
  it("renders WBS, phase summary, and sprint prognosis from live data", async () => {
    renderWithClient(<ReportView />);

    expect(await screen.findByRole("heading", { name: "WBS Report" })).toBeInTheDocument();
    expect(screen.getByText("US-F1-001")).toBeInTheDocument();
    expect(screen.getByText("Todo story")).toBeInTheDocument();
    expect(screen.getAllByText("MP1 - Foundation").length).toBeGreaterThan(0);
    expect(screen.getByText(/P50 2026-06-15 \/ P80 2026-06-16 \/ P90 2026-06-17/)).toBeInTheDocument();
    expect(screen.getByText("S000.start")).toBeInTheDocument();
    expect(screen.getAllByText(/daily throughput over 3 observed workdays/).length).toBeGreaterThan(0);
  });
});
