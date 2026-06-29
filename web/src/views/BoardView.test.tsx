import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import type { RepositorySnapshot } from "@shared/types.js";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BoardView } from "./BoardView.js";

function snapshot(): RepositorySnapshot {
  const activeStory = {
    id: "US-F1-061", title: "Frontend", status: "in-progress",
    phase: "F1", epic: "EP-F1-07", sprint: "S000.start", priority: null, storyPoints: 5,
    assignee: "Sondre Bjerkerud <sondre@example.com>, Erik Itland <erik@example.com>",
    assignees: ["Sondre Bjerkerud <sondre@example.com>", "Erik Itland <erik@example.com>"],
    workStarted: null, workDone: null, activated: null, created: null, updated: null,
    relativePath: "x", tasks: [], taskSummary: { todo: 0, inProgress: 0, readyForQa: 0, done: 2, blocked: 0, total: 4 }, frontmatter: {},
  };
  const plannedStory = {
    ...activeStory,
    id: "US-F1-062",
    title: "Next",
    status: "planned",
    sprint: "S001.next",
  };
  return {
    stories: [activeStory, plannedStory], epics: [],
    sprints: [
      { name: "S001.next", id: "S001", headline: "next", goal: null, startDate: "2026-06-01", endDate: "2026-06-14", status: "planned", wipLimit: null, storiesByStatus: { planned: [plannedStory], todo: [], "in-progress": [], "ready-for-qa": [], done: [], blocked: [] } },
      { name: "S000.start", id: "S000", headline: "start", goal: null, startDate: "2026-05-18", endDate: "2026-05-31", status: "active", wipLimit: null, storiesByStatus: { planned: [], todo: [], "in-progress": [activeStory], "ready-for-qa": [], done: [], blocked: [] } },
    ],
    progress: { donePoints: 0, totalPoints: 10, doneStories: 0, totalStories: 2, phases: [] },
  };
}

function renderWithClient(ui: ReactNode) {
  const qc = new QueryClient();
  qc.setQueryData(["repository"], snapshot());
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async (url: string) => {
    if (url === "/api/team") return new Response(JSON.stringify([]), { status: 200 });
    return new Response(JSON.stringify(snapshot()), { status: 200 });
  }));
});

describe("BoardView", () => {
  it("renders the active sprint columns and a story card", async () => {
    renderWithClient(<BoardView />);
    expect(await screen.findByText("US-F1-061")).toBeInTheDocument();
    expect(screen.getByText("In Progress")).toBeInTheDocument();
    expect(screen.getByText("2/4 done")).toBeInTheDocument();
    // Avatar initials fallback: first 2 chars uppercased (no team roster in test)
    expect(screen.getByTitle("Sondre Bjerkerud <sondre@example.com>")).toBeInTheDocument();
    expect(screen.getByTitle("Erik Itland <erik@example.com>")).toBeInTheDocument();
    expect(screen.getByLabelText("sprint")).toHaveValue("S000.start");
    expect(screen.queryByText("US-F1-062")).not.toBeInTheDocument();
  });

  it("switches the board to another selected sprint", async () => {
    renderWithClient(<BoardView />);
    fireEvent.change(await screen.findByLabelText("sprint"), { target: { value: "S001.next" } });
    expect(screen.getByText("US-F1-062")).toBeInTheDocument();
    expect(screen.queryByText("US-F1-061")).not.toBeInTheDocument();
  });
});
