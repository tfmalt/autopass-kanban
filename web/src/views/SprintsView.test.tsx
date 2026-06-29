import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { RepositorySnapshot } from "@shared/types.js";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SprintsView } from "./SprintsView.js";

function snapshot(): RepositorySnapshot {
  return {
    stories: [], epics: [],
    sprints: [{ name: "S000.start", id: "S000", headline: "start", goal: "**Important**\n\n- first item\n- second item", startDate: "2026-05-18", endDate: "2026-05-31", status: "active", wipLimit: null, storiesByStatus: { planned: [], todo: [], "in-progress": [], "ready-for-qa": [], done: [], blocked: [] } }],
    progress: { donePoints: 0, totalPoints: 0, doneStories: 0, totalStories: 0, phases: [] },
  };
}

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async (url: string) => {
    if (url === "/api/sprints") return new Response(JSON.stringify({ ok: true }), { status: 200 });
    if (url === "/api/sprints/S000.start") {
      return new Response(JSON.stringify({ ok: true, data: { name: "S000.renamed", headline: "renamed", sprintPath: "/repo/delivery/sprints/S000.renamed.md" } }), { status: 200 });
    }
    return new Response(JSON.stringify(snapshot()), { status: 200 });
  }));
});

function renderWithClient(ui: ReactNode) {
  const qc = new QueryClient();
  qc.setQueryData(["repository"], snapshot());
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

describe("SprintsView", () => {
  it("lists sprints and submits the create form", async () => {
    renderWithClient(<SprintsView />);
    expect(await screen.findByText("S000.start")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /create sprint/i }));
    fireEvent.change(screen.getByLabelText(/headline/i), { target: { value: "planning" } });
    fireEvent.click(screen.getByRole("button", { name: /^create$/i }));
    await waitFor(() => expect(fetch).toHaveBeenCalledWith("/api/sprints", expect.objectContaining({ method: "POST" })));
  });

  it("opens a sprint drawer in read mode before editing", async () => {
    renderWithClient(<SprintsView />);

    expect(screen.queryByRole("button", { name: /^edit$/i })).not.toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: /open sprint S000\.start/i }));

    expect(screen.getByRole("heading", { name: "S000.start" })).toBeInTheDocument();
    expect(screen.getByText("Sprint goal")).toBeInTheDocument();
    expect(screen.queryByLabelText(/edit sprint goal/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^edit$/i }));
    fireEvent.change(screen.getByLabelText(/edit sprint headline/i), { target: { value: "Renamed" } });
    fireEvent.change(screen.getByLabelText(/edit sprint goal/i), { target: { value: "Updated sprint goal" } });
    fireEvent.change(screen.getByLabelText(/edit status/i), { target: { value: "closed" } });
    fireEvent.click(screen.getByRole("button", { name: /^save$/i }));

    await waitFor(() => expect(fetch).toHaveBeenCalledWith(
      "/api/sprints/S000.start",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          headline: "Renamed",
          goal: "Updated sprint goal",
          start: "2026-05-18",
          end: "2026-05-31",
          status: "closed",
          wipLimit: null,
        }),
      }),
    ));
  });

  it("renders sprint goals with markdown formatting", async () => {
    renderWithClient(<SprintsView />);

    fireEvent.click(await screen.findByRole("button", { name: /open sprint S000\.start/i }));

    expect(screen.getByText("Important", { selector: "strong" })).toBeInTheDocument();
    expect(screen.getByText("first item")).toBeInTheDocument();
    expect(screen.getByText("second item")).toBeInTheDocument();
  });
});
