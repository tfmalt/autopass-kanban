import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import type { DashboardMetrics, RepositorySnapshot, Story } from "@shared/types.js";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DashboardView } from "./DashboardView.js";

function metrics(): DashboardMetrics {
  return {
    burnup: [
      { date: "2026-05-20", completed: 0, scope: 13 },
      { date: "2026-05-22", completed: 1, scope: 13 },
      { date: "2026-05-25", completed: 5, scope: 13 },
    ],
    burndown: [{ date: "2026-05-20", remaining: 13, ideal: 13 }, { date: "2026-05-25", remaining: 8, ideal: 6 }],
    leadTime: [{ storyId: "US-F1-001", date: "2026-05-25", days: 5, rollingAvg: 5 }],
    velocity: [{ sprint: "S000.start", points: 5, forecast: false }],
    forecast: {
      generatedAt: "2026-05-25T10:00:00Z",
      remainingPoints: 8,
      sprintDurationWeeks: 2,
      projectionStartDate: "2026-05-25",
      throughput: { samples: [0, 5], average: 2.5, median: 2.5, observedDayCount: 2 },
      completion: {
        p50Days: 2,
        p80Days: 2,
        p90Days: 3,
        p50Date: "2026-06-22",
        p80Date: "2026-06-22",
        p90Date: "2026-07-06",
      },
      confidence: "low",
    },
    progress: { donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2, phases: [{ phase: "F1", donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2 }] },
  };
}

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
  const done = story({ id: "US-F1-001", title: "Done", status: "done", storyPoints: 5 });
  const todo = story({ id: "US-F1-002", title: "Todo", status: "todo", storyPoints: 8 });
  return {
    stories: [done, todo],
    epics: [{ id: "EP-F1-01", title: "Platform", phase: "F1", priority: null, stories: [done, todo] }],
    sprints: [],
    progress: { donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2, phases: [{ phase: "F1", donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2 }] },
  };
}

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async (url: string) => {
    if (url.includes("/api/repository")) return new Response(JSON.stringify(repository()), { status: 200 });
    return new Response(JSON.stringify(metrics()), { status: 200 });
  }));
});

function renderWithClient(ui: ReactNode) {
  const qc = new QueryClient();
  qc.setQueryData(["metrics"], metrics());
  qc.setQueryData(["repository"], repository());
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

describe("DashboardView", () => {
  it("shows the projected end date and per-phase breakdown", async () => {
    renderWithClient(<DashboardView />);
    expect((await screen.findAllByText(/2026-06-22/)).length).toBeGreaterThan(0);
    expect(screen.getByText(/Canonical forecast: P50 2026-06-22 \/ P80 2026-06-22 \/ P90 2026-07-06/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /F1/ })).toBeInTheDocument();
  });

  it("expands a phase row to show epic progress", async () => {
    renderWithClient(<DashboardView />);
    expect(screen.queryByText(/EP-F1-01/)).not.toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: /F1/ }));

    expect(screen.getByText(/EP-F1-01: Platform/)).toBeInTheDocument();
    expect(screen.getAllByText(/38%/).length).toBeGreaterThan(0);
  });
});
