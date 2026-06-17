import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { RepositorySnapshot, Story } from "@shared/types.js";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BacklogView } from "./BacklogView.js";

function makeStory(overrides: Partial<Story> & Pick<Story, "id" | "title">): Story {
  return {
    id: overrides.id,
    title: overrides.title,
    status: overrides.status ?? "todo",
    phase: overrides.phase ?? "F2",
    epic: overrides.epic === undefined ? "EP-F2-01" : overrides.epic,
    sprint: overrides.sprint ?? null,
    priority: overrides.priority ?? null,
    storyPoints: overrides.storyPoints ?? 3,
    assignee: overrides.assignee ?? null,
    workStarted: overrides.workStarted ?? null,
    workDone: overrides.workDone ?? null,
    assignees: overrides.assignees ?? [],
    activated: overrides.activated ?? null,
    created: overrides.created ?? null,
    updated: overrides.updated ?? null,
    relativePath: overrides.relativePath ?? `${overrides.id}.md`,
    tasks: overrides.tasks ?? [],
    taskSummary: overrides.taskSummary ?? { todo: 0, inProgress: 0, readyForQa: 0, done: 0, blocked: 0, total: 0 },
    frontmatter: overrides.frontmatter ?? {},
  };
}

function snapshot(): RepositorySnapshot {
  const sprintHigh = makeStory({ id: "US-F2-050", title: "Sprint high", epic: "EP-F2-02", sprint: "S001.plan", priority: 10, storyPoints: 5 });
  const sprintLow = makeStory({ id: "US-F2-051", title: "Sprint low", epic: "EP-F2-02", sprint: "S001.plan", priority: 20, status: "in-progress", storyPoints: 2 });
  const epicOneHigh = makeStory({ id: "US-F2-002", title: "Backlog high", epic: "EP-F2-01", priority: 10, storyPoints: 8 });
  const epicOneLow = makeStory({ id: "US-F2-001", title: "Backlog low", epic: "EP-F2-01", priority: 20, storyPoints: 3 });
  const epicTwo = makeStory({ id: "US-F2-003", title: "Other epic story", epic: "EP-F2-02", priority: 30, storyPoints: 5 });
  const noEpic = makeStory({ id: "US-F2-004", title: "No epic story", epic: null, priority: 40, storyPoints: 1 });

  return {
    stories: [epicOneLow, epicTwo, sprintLow, epicOneHigh, sprintHigh, noEpic],
    epics: [
      { id: "EP-F2-02", title: "Second Epic", phase: "F2", priority: 10, stories: [epicTwo, sprintHigh, sprintLow] },
      { id: "EP-F2-01", title: "Platform Epic", phase: "F2", priority: 20, stories: [epicOneLow, epicOneHigh] },
      { id: "EP-F2-03", title: "Empty Epic", phase: "F2", priority: 30, stories: [] },
    ],
    sprints: [{
      name: "S001.plan",
      id: "S001",
      headline: "plan",
      goal: null,
      startDate: "2026-06-01",
      endDate: "2026-06-12",
      status: "planned",
      wipLimit: null,
      storiesByStatus: {
        todo: [sprintHigh],
        "in-progress": [sprintLow],
        "ready-for-qa": [],
        done: [],
        blocked: [],
      },
    }],
    progress: { donePoints: 0, totalPoints: 24, doneStories: 0, totalStories: 6, phases: [] },
  };
}

beforeEach(() => {
  let data = snapshot();
  vi.stubGlobal("fetch", vi.fn(async (input: string, init?: RequestInit) => {
    if (input.includes("/api/stories/") && input.includes("/fields") && init?.method === "PATCH") {
      const id = decodeURIComponent(input.split("/api/stories/")[1]!.split("/fields")[0]!);
      const body = JSON.parse(String(init.body ?? "{}")) as { sprint?: string; status?: string; priority?: number };
      const story = data.stories.find((candidate) => candidate.id === id)!;
      const updatedStory = {
        ...story,
        ...(body.priority !== undefined ? { priority: body.priority } : {}),
        ...(body.sprint !== undefined ? { sprint: body.sprint || null } : {}),
        ...(body.status !== undefined ? { status: body.status } : {}),
      };

      data = {
        ...data,
        stories: data.stories.map((candidate) => candidate.id === id ? updatedStory : candidate),
        epics: data.epics.map((epic) => ({
          ...epic,
          stories: epic.stories.map((candidate) => candidate.id === id ? updatedStory : candidate),
        })),
        sprints: data.sprints.map((sprint) => ({
          ...sprint,
          storiesByStatus: {
            todo: sprint.storiesByStatus.todo
              .filter((candidate) => candidate.id !== id)
              .concat(updatedStory.sprint === sprint.name && updatedStory.status === "todo" ? [updatedStory] : []),
            "in-progress": sprint.storiesByStatus["in-progress"]
              .filter((candidate) => candidate.id !== id)
              .concat(updatedStory.sprint === sprint.name && updatedStory.status === "in-progress" ? [updatedStory] : []),
            "ready-for-qa": sprint.storiesByStatus["ready-for-qa"].filter((candidate) => candidate.id !== id),
            done: sprint.storiesByStatus.done.filter((candidate) => candidate.id !== id),
            blocked: sprint.storiesByStatus.blocked.filter((candidate) => candidate.id !== id),
          },
        })),
      };

      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }

    if (input.includes("/api/epics/") && input.endsWith("/fields") && init?.method === "PATCH") {
      const id = decodeURIComponent(input.split("/api/epics/")[1]!.split("/fields")[0]!);
      const body = JSON.parse(String(init.body ?? "{}")) as { priority: number };
      data = {
        ...data,
        epics: data.epics.map((epic) => epic.id === id ? { ...epic, priority: body.priority } : epic),
      };
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }

    if (input.includes("/api/epics/EP-F2-01")) {
      return new Response(JSON.stringify({
        id: "EP-F2-01",
        title: "Platform Epic",
        phase: "F2",
        priority: 20,
        stories: [],
        body: [
          "# Epic: Platform Epic",
          "",
          "---",
          "",
          "## Forretningskontekst",
          "",
          "Dette er den **viktigste** konteksten for denne epicen.",
          "",
          "---",
          "",
          "## Forretningsverdi",
          "",
          "Dette forklarer verdien som backlog-visningen skal vise:",
          "",
          "- Synlig kontekst",
          "- Bevart formatering",
          "",
          "## Avhengigheter",
          "",
          "- Ignored list item",
        ].join("\n"),
      }), { status: 200 });
    }

    if (input.includes("/api/stories/") && !input.includes("/plan") && !input.includes("/fields")) {
      const id = decodeURIComponent(input.split("/api/stories/")[1]!);
      const story = data.stories.find((candidate) => candidate.id === id);
      return new Response(JSON.stringify({ ...(story ?? data.stories[0]), body: "# Story\n\nDetails" }), { status: 200 });
    }

    if (input.includes("/api/config")) {
      return new Response(JSON.stringify({ port: 3000, host: "localhost", style: "calm", version: "test", branch: "test", storyPoints: { allowedValues: ["8"], aliases: {} } }), { status: 200 });
    }

    if (input.includes("/api/team")) {
      return new Response(JSON.stringify([]), { status: 200 });
    }

    if (input.includes("/plan")) {
      const id = decodeURIComponent(input.split("/api/stories/")[1]!.split("/plan")[0]!);
      const story = data.stories.find((candidate) => candidate.id === id)!;
      const plannedStory = { ...story, status: "todo", sprint: "S001.plan" };
      data = {
        ...data,
        stories: data.stories.map((candidate) => candidate.id === id ? plannedStory : candidate),
        epics: data.epics.map((epic) => ({
          ...epic,
          stories: epic.stories.map((candidate) => candidate.id === id ? plannedStory : candidate),
        })),
        sprints: data.sprints.map((sprint) =>
          sprint.name === "S001.plan"
            ? { ...sprint, storiesByStatus: { ...sprint.storiesByStatus, todo: [...sprint.storiesByStatus.todo.filter((candidate) => candidate.id !== id), plannedStory] } }
            : sprint,
        ),
      };
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }

    return new Response(JSON.stringify(data), { status: 200 });
  }));
});

function renderWithClient(ui: ReactNode) {
  const qc = new QueryClient();
  qc.setQueryData(["repository"], snapshot());
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

function storyIdsIn(container: HTMLElement) {
  return within(container)
    .queryAllByTestId(/story-card-/)
    .map((card) => card.getAttribute("data-testid")!.replace("story-card-", ""));
}

function epicIdsInBacklog() {
  return screen
    .queryAllByTestId(/epic-section-/)
    .filter((section) => section.getAttribute("data-testid") !== "epic-section-no-epic")
    .map((section) => section.getAttribute("data-testid")!.replace("epic-section-", ""));
}

describe("BacklogView", () => {
  it("lists unplanned stories by epic and plans one into the selected sprint", async () => {
    renderWithClient(<BacklogView />);
    expect(await screen.findByText("US-F2-001")).toBeInTheDocument();
    expect(screen.getByLabelText("current sprint drop target")).toHaveTextContent("Sprint high");
    fireEvent.click(screen.getByRole("button", { name: /add US-F2-001/i }));
    await waitFor(() => expect(fetch).toHaveBeenCalledWith(expect.stringContaining("/plan"), expect.objectContaining({ method: "POST" })));
    await waitFor(() => expect(screen.queryByRole("button", { name: /add US-F2-001/i })).not.toBeInTheDocument());
    expect(screen.getByLabelText("current sprint drop target")).toHaveTextContent("US-F2-001");
  });

  it("removes a planned story from the selected sprint", async () => {
    renderWithClient(<BacklogView />);
    const dropTarget = await screen.findByLabelText("current sprint drop target");
    expect(await within(dropTarget).findByRole("button", { name: /remove US-F2-050/i })).toBeInTheDocument();

    fireEvent.click(within(dropTarget).getByRole("button", { name: /remove US-F2-050/i }));

    await waitFor(() => expect(fetch).toHaveBeenCalledWith(expect.stringContaining("/fields"), expect.objectContaining({ method: "PATCH" })));
    await waitFor(() => expect(within(dropTarget).queryByText("US-F2-050")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: /add US-F2-050/i })).toBeInTheDocument();
  });

  it("opens the story sidebar from a backlog story click", async () => {
    renderWithClient(<BacklogView />);
    fireEvent.click(await screen.findByText("Backlog low"));
    expect(await screen.findByText("Details")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Edit" })).toBeInTheDocument();
  });

  it("renders stories within an epic in priority order", async () => {
    renderWithClient(<BacklogView />);
    const epicSection = await screen.findByTestId("epic-section-EP-F2-01");
    const sectionStories = within(epicSection)
      .queryAllByTestId(/story-card-/)
      .map((card) => card.getAttribute("data-testid")!.replace("story-card-", ""));
    expect(sectionStories).toEqual(["US-F2-002", "US-F2-001"]);
  });

  it("renders epics in priority order", async () => {
    renderWithClient(<BacklogView />);
    await screen.findByTestId("epic-section-EP-F2-01");
    expect(epicIdsInBacklog()).toEqual(["EP-F2-02", "EP-F2-01", "EP-F2-03"]);
  });

  it("renders sprint stories in priority order", async () => {
    renderWithClient(<BacklogView />);
    const dropTarget = await screen.findByLabelText("current sprint drop target");
    expect(storyIdsIn(dropTarget)).toEqual(["US-F2-050", "US-F2-051"]);
  });

  it("shows the epic title and collapses then expands epic context on click", async () => {
    renderWithClient(<BacklogView />);
    const epicButton = await screen.findByRole("button", { name: /EP-F2-01.*Platform Epic/i });
    expect(epicButton).toBeInTheDocument();
    expect(await screen.findByText("Forretningskontekst")).toBeInTheDocument();

    fireEvent.click(epicButton);
    await waitFor(() => expect(screen.queryByText("Forretningskontekst")).not.toBeInTheDocument());
    expect(epicButton).toHaveTextContent("(2 stories)");

    fireEvent.click(epicButton);
    expect(await screen.findByText("Forretningskontekst")).toBeInTheDocument();
    expect(screen.getByText("viktigste")).toBeInTheDocument();
    expect(screen.getByText("Forretningsverdi")).toBeInTheDocument();
    expect(screen.getByText("Synlig kontekst")).toBeInTheDocument();
    expect(screen.getByText("Bevart formatering")).toBeInTheDocument();
  });

  it("disables backlog drag reorder while search is active without disabling the plus button", async () => {
    renderWithClient(<BacklogView />);
    const search = await screen.findByPlaceholderText("Search stories...");
    fireEvent.change(search, { target: { value: "backlog" } });

    expect(screen.getByText(/Priority reordering is disabled while filtering/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /add US-F2-001/i })).toBeEnabled();
    expect(screen.getByLabelText("reorder epic EP-F2-01")).toHaveAttribute("aria-disabled", "true");
  });

  it("keeps the no epic group out of epic fetching and epic sorting", async () => {
    renderWithClient(<BacklogView />);
    expect(await screen.findByTestId("epic-section-no-epic")).toBeInTheDocument();
    expect(screen.getByText("(no epic)")).toBeInTheDocument();

    fireEvent.click(screen.getByText("No epic story"));
    expect(await screen.findByText("Details")).toBeInTheDocument();

    expect(fetch).not.toHaveBeenCalledWith(expect.stringContaining("/api/epics/(no%20epic)"), expect.anything());
    expect(fetch).not.toHaveBeenCalledWith(expect.stringContaining("/api/epics/(no epic)"), expect.anything());
  });

  it("clears the active overlay state on drag cancel", async () => {
    renderWithClient(<BacklogView />);
    const handle = await screen.findByLabelText("reorder epic EP-F2-01");

    fireEvent.keyDown(handle, { key: " ", code: "Space" });

    await waitFor(() => expect(screen.getByTestId("backlog-drag-overlay")).toBeInTheDocument());

    fireEvent.keyDown(handle, { key: "Escape", code: "Escape" });

    await waitFor(() => expect(screen.queryByTestId("backlog-drag-overlay")).not.toBeInTheDocument());
  });
});
