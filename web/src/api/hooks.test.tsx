import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { Epic, RepositorySnapshot, Story } from "@shared/types.js";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { byPriorityThenId, computePriorityUpdates, useMoveStory, useReorderEpics, useReorderStories, useUnplanStory } from "./hooks.js";

// ---------------------------------------------------------------------------
// Module mock — must come before any imports that exercise the module under test.
// ---------------------------------------------------------------------------
vi.mock("./client.js", () => ({
  fetchRepository: vi.fn(),
  fetchMetrics: vi.fn(),
  fetchConfig: vi.fn(),
  fetchTeam: vi.fn(),
  fetchStory: vi.fn(),
  fetchEpic: vi.fn(),
  createSprint: vi.fn(),
  updateSprint: vi.fn(),
  updateStory: vi.fn(),
  planStory: vi.fn(),
  moveStory: vi.fn(),
  updateEpicFields: vi.fn(),
  updateStoryFields: vi.fn(),
  updateTaskStatus: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Import mocked functions AFTER vi.mock so we get the mock instances.
// ---------------------------------------------------------------------------
import { moveStory, updateEpicFields, updateStoryFields } from "./client.js";

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------
function makeStory(overrides: Partial<Story> & Pick<Story, "id" | "status">): Story {
  const {
    id,
    status,
    title,
    phase,
    epic,
    sprint,
    priority,
    storyPoints,
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
  } = overrides;
  return {
    id,
    status,
    title: title ?? id,
    phase: phase ?? "F1",
    epic: epic ?? "EP-F1-01",
    sprint: sprint ?? null,
    priority: priority ?? null,
    storyPoints: storyPoints ?? 3,
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

function makeEpic(overrides: Partial<Epic> & Pick<Epic, "id">): Epic {
  const { id, title, phase, priority, stories } = overrides;
  return {
    id,
    title: title ?? id,
    phase: phase ?? "F1",
    priority: priority ?? null,
    stories: stories ?? [],
  };
}

function makeSnapshot(
  sprintStory: Story,
  backlogStory: Story,
): RepositorySnapshot {
  return {
    stories: [sprintStory, backlogStory],
    epics: [
      {
        id: "EP-F1-01",
        title: "Platform",
        phase: "F1",
        priority: 10,
        stories: [sprintStory, backlogStory],
      },
    ],
    sprints: [
      {
        name: "S000.start",
        id: "S000",
        headline: "start",
        goal: null,
        startDate: "2026-05-18",
        endDate: "2026-05-31",
        status: "active",
        wipLimit: null,
        storiesByStatus: {
          planned: [],
          todo: [],
          "in-progress": [sprintStory],
          "ready-for-qa": [],
          done: [],
          blocked: [],
        },
      },
    ],
    progress: {
      donePoints: 0,
      totalPoints: 6,
      doneStories: 0,
      totalStories: 2,
      phases: [],
    },
  };
}

function makePrioritySnapshot(): RepositorySnapshot {
  const storyA = makeStory({ id: "US-F1-001", status: "todo", priority: 10, sprint: "S000.start" });
  const storyB = makeStory({ id: "US-F1-002", status: "todo", priority: 20, sprint: "S000.start" });
  const storyC = makeStory({ id: "US-F1-003", status: "todo", priority: 30, sprint: null });
  const storyD = makeStory({ id: "US-F1-004", status: "todo", priority: null, sprint: null });
  const epic1 = makeEpic({ id: "EP-F1-01", title: "Alpha", priority: 10, stories: [storyA, storyB] });
  const epic2 = makeEpic({ id: "EP-F1-02", title: "Beta", priority: 20, stories: [storyC, storyD] });
  return {
    stories: [storyA, storyB, storyC, storyD],
    epics: [epic1, epic2],
    sprints: [
      {
        name: "S000.start",
        id: "S000",
        headline: "start",
        goal: null,
        startDate: "2026-05-18",
        endDate: "2026-05-31",
        status: "active",
        wipLimit: null,
        storiesByStatus: {
          planned: [],
          todo: [storyA, storyB],
          "in-progress": [],
          "ready-for-qa": [],
          done: [],
          blocked: [],
        },
      },
    ],
    progress: {
      donePoints: 0,
      totalPoints: 12,
      doneStories: 0,
      totalStories: 4,
      phases: [],
    },
  };
}

// ---------------------------------------------------------------------------
// QueryClient factory — retries disabled to avoid hanging on rejection tests.
// ---------------------------------------------------------------------------
function makeQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });
}

function wrapper(qc: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
  };
}

describe("computePriorityUpdates", () => {
  it("assigns 10 for a single unranked item", () => {
    expect(computePriorityUpdates(["US-F1-001"], "US-F1-001", [{ id: "US-F1-001", priority: null }])).toEqual([
      { id: "US-F1-001", priority: 10 },
    ]);
  });

  it("handles drop before first ranked item", () => {
    expect(
      computePriorityUpdates(
        ["US-F1-003", "US-F1-001", "US-F1-002"],
        "US-F1-003",
        [
          { id: "US-F1-001", priority: 10 },
          { id: "US-F1-002", priority: 20 },
          { id: "US-F1-003", priority: 30 },
        ],
      ),
    ).toEqual([{ id: "US-F1-003", priority: 5 }]);
  });

  it("handles drop after last ranked item", () => {
    expect(
      computePriorityUpdates(
        ["US-F1-002", "US-F1-003", "US-F1-001"],
        "US-F1-001",
        [
          { id: "US-F1-001", priority: 10 },
          { id: "US-F1-002", priority: 20 },
          { id: "US-F1-003", priority: 30 },
        ],
      ),
    ).toEqual([{ id: "US-F1-001", priority: 40 }]);
  });

  it("normalizes when any item is unranked", () => {
    expect(
      computePriorityUpdates(
        ["US-F1-002", "US-F1-001", "US-F1-003"],
        "US-F1-002",
        [
          { id: "US-F1-001", priority: 10 },
          { id: "US-F1-002", priority: null },
          { id: "US-F1-003", priority: 30 },
        ],
      ),
    ).toEqual([
      { id: "US-F1-002", priority: 10 },
      { id: "US-F1-001", priority: 20 },
    ]);
  });

  it("normalizes when gap is exhausted", () => {
    expect(
      computePriorityUpdates(
        ["US-F1-001", "US-F1-003", "US-F1-002"],
        "US-F1-003",
        [
          { id: "US-F1-001", priority: 10 },
          { id: "US-F1-002", priority: 11 },
          { id: "US-F1-003", priority: 30 },
        ],
      ),
    ).toEqual([
      { id: "US-F1-003", priority: 20 },
      { id: "US-F1-002", priority: 30 },
    ]);
  });
});

describe("byPriorityThenId", () => {
  it("sorts by priority then id", () => {
    expect(
      byPriorityThenId([
        { id: "b", priority: null },
        { id: "c", priority: 10 },
        { id: "a", priority: 10 },
      ]),
    ).toEqual([
      { id: "a", priority: 10 },
      { id: "c", priority: 10 },
      { id: "b", priority: null },
    ]);
  });
});

// ---------------------------------------------------------------------------
// Tests: useMoveStory
// ---------------------------------------------------------------------------
describe("useMoveStory", () => {
  let qc: QueryClient;
  let sprintStory: Story;
  let backlogStory: Story;
  let snapshot: RepositorySnapshot;

  beforeEach(() => {
    vi.resetAllMocks();
    qc = makeQueryClient();
    sprintStory = makeStory({ id: "US-F1-001", status: "in-progress", sprint: "S000.start" });
    backlogStory = makeStory({ id: "US-F1-002", status: "todo", sprint: null });
    snapshot = makeSnapshot(sprintStory, backlogStory);
    qc.setQueryData<RepositorySnapshot>(["repository"], snapshot);
  });

  it("optimistically moves a story to the new status bucket before the server responds", async () => {
    // Hang the network call indefinitely so we can inspect the optimistic state.
    let resolveMove!: () => void;
    vi.mocked(moveStory).mockReturnValue(
      new Promise<void>((resolve) => {
        resolveMove = resolve;
      }),
    );

    const { result } = renderHook(() => useMoveStory(), { wrapper: wrapper(qc) });

    act(() => {
      result.current.mutate({ id: "US-F1-001", status: "done" });
    });

    // Give react-query a tick to run onMutate
    await act(async () => {
      await Promise.resolve();
    });

    const optimistic = qc.getQueryData<RepositorySnapshot>(["repository"])!;

    // Story in top-level list should reflect new status
    const topLevel = optimistic.stories.find((s) => s.id === "US-F1-001")!;
    expect(topLevel.status).toBe("done");

    // Story should be in the "done" sprint bucket
    const sprint = optimistic.sprints[0]!;
    expect(sprint.storiesByStatus.done.some((s) => s.id === "US-F1-001")).toBe(true);

    // Story should no longer be in "in-progress" bucket
    expect(sprint.storiesByStatus["in-progress"].some((s) => s.id === "US-F1-001")).toBe(false);

    // Epic stories should also reflect new status
    const epicStory = optimistic.epics[0]!.stories.find((s) => s.id === "US-F1-001")!;
    expect(epicStory.status).toBe("done");

    // Resolve network call so the mutation can settle without leaks
    resolveMove();
    await waitFor(() => expect(result.current.isIdle).toBe(true));
  });

  it("rolls back the optimistic update on server error", async () => {
    vi.mocked(moveStory).mockRejectedValue(new Error("server error"));

    const { result } = renderHook(() => useMoveStory(), { wrapper: wrapper(qc) });

    await act(async () => {
      result.current.mutate({ id: "US-F1-001", status: "done" });
    });

    await waitFor(() => expect(result.current.isError).toBe(true));

    const current = qc.getQueryData<RepositorySnapshot>(["repository"])!;
    const topLevel = current.stories.find((s) => s.id === "US-F1-001")!;
    expect(topLevel.status).toBe("in-progress");

    const sprint = current.sprints[0]!;
    expect(sprint.storiesByStatus["in-progress"].some((s) => s.id === "US-F1-001")).toBe(true);
    expect(sprint.storiesByStatus.done.some((s) => s.id === "US-F1-001")).toBe(false);
  });

  it("also updates the assignee optimistically when provided", async () => {
    let resolveMove!: () => void;
    vi.mocked(moveStory).mockReturnValue(
      new Promise<void>((resolve) => {
        resolveMove = resolve;
      }),
    );

    const { result } = renderHook(() => useMoveStory(), { wrapper: wrapper(qc) });

    act(() => {
      result.current.mutate({
        id: "US-F1-001",
        status: "done",
        assignee: "Alice <alice@example.com>",
      });
    });

    await act(async () => {
      await Promise.resolve();
    });

    const optimistic = qc.getQueryData<RepositorySnapshot>(["repository"])!;
    const topLevel = optimistic.stories.find((s) => s.id === "US-F1-001")!;
    expect(topLevel.assignee).toBe("Alice <alice@example.com>");
    expect(topLevel.assignees).toContain("Alice <alice@example.com>");

    resolveMove();
    await waitFor(() => expect(result.current.isIdle).toBe(true));
  });
});

// ---------------------------------------------------------------------------
// Tests: useUnplanStory
// ---------------------------------------------------------------------------
describe("useUnplanStory", () => {
  let qc: QueryClient;
  let sprintStory: Story;
  let backlogStory: Story;
  let snapshot: RepositorySnapshot;

  beforeEach(() => {
    vi.resetAllMocks();
    qc = makeQueryClient();
    sprintStory = makeStory({ id: "US-F1-001", status: "in-progress", sprint: "S000.start" });
    backlogStory = makeStory({ id: "US-F1-002", status: "todo", sprint: null });
    snapshot = makeSnapshot(sprintStory, backlogStory);
    qc.setQueryData<RepositorySnapshot>(["repository"], snapshot);
  });

  it("optimistically removes story from all sprint buckets and sets sprint to null", async () => {
    let resolveUpdate!: () => void;
    vi.mocked(updateStoryFields).mockReturnValue(
      new Promise<void>((resolve) => {
        resolveUpdate = resolve;
      }),
    );

    const { result } = renderHook(() => useUnplanStory(), { wrapper: wrapper(qc) });

    act(() => {
      result.current.mutate({ id: "US-F1-001" });
    });

    await act(async () => {
      await Promise.resolve();
    });

    const optimistic = qc.getQueryData<RepositorySnapshot>(["repository"])!;

    // Top-level story: sprint should be null and status should be "ready"
    const topLevel = optimistic.stories.find((s) => s.id === "US-F1-001")!;
    expect(topLevel.sprint).toBeNull();
    expect(topLevel.status).toBe("ready");

    // Story should be absent from every sprint bucket
    const sprint = optimistic.sprints[0]!;
    const allBucketIds = [
      ...sprint.storiesByStatus.planned,
      ...sprint.storiesByStatus.todo,
      ...sprint.storiesByStatus["in-progress"],
      ...sprint.storiesByStatus["ready-for-qa"],
      ...sprint.storiesByStatus.done,
      ...sprint.storiesByStatus.blocked,
    ].map((s) => s.id);
    expect(allBucketIds).not.toContain("US-F1-001");

    // Epic stories should also reflect the change
    const epicStory = optimistic.epics[0]!.stories.find((s) => s.id === "US-F1-001")!;
    expect(epicStory.sprint).toBeNull();
    expect(epicStory.status).toBe("ready");

    resolveUpdate();
    await waitFor(() => expect(result.current.isIdle).toBe(true));
  });

  it("rolls back optimistic unplan on server error", async () => {
    vi.mocked(updateStoryFields).mockRejectedValue(new Error("server error"));

    const { result } = renderHook(() => useUnplanStory(), { wrapper: wrapper(qc) });

    await act(async () => {
      result.current.mutate({ id: "US-F1-001" });
    });

    await waitFor(() => expect(result.current.isError).toBe(true));

    const current = qc.getQueryData<RepositorySnapshot>(["repository"])!;
    const topLevel = current.stories.find((s) => s.id === "US-F1-001")!;
    expect(topLevel.sprint).toBe("S000.start");
    expect(topLevel.status).toBe("in-progress");

    const sprint = current.sprints[0]!;
    expect(sprint.storiesByStatus["in-progress"].some((s) => s.id === "US-F1-001")).toBe(true);
  });

  it("does not touch other stories when unplanning one", async () => {
    // Place backlogStory also in the sprint todo bucket to verify only target is removed
    const sprintStoryWithSibling = makeStory({ id: "US-F1-003", status: "todo", sprint: "S000.start" });
    const snapshotWithSibling: RepositorySnapshot = {
      ...snapshot,
      stories: [...snapshot.stories, sprintStoryWithSibling],
      sprints: [
        {
          ...snapshot.sprints[0]!,
          storiesByStatus: {
            ...snapshot.sprints[0]!.storiesByStatus,
            "in-progress": [sprintStory],
            todo: [sprintStoryWithSibling],
          },
        },
      ],
    };
    qc.setQueryData<RepositorySnapshot>(["repository"], snapshotWithSibling);

    let resolveUpdate!: () => void;
    vi.mocked(updateStoryFields).mockReturnValue(
      new Promise<void>((resolve) => {
        resolveUpdate = resolve;
      }),
    );

    const { result } = renderHook(() => useUnplanStory(), { wrapper: wrapper(qc) });

    act(() => {
      result.current.mutate({ id: "US-F1-001" });
    });

    await act(async () => {
      await Promise.resolve();
    });

    const optimistic = qc.getQueryData<RepositorySnapshot>(["repository"])!;
    const sprint = optimistic.sprints[0]!;

    // US-F1-001 should be gone from in-progress
    expect(sprint.storiesByStatus["in-progress"].some((s) => s.id === "US-F1-001")).toBe(false);
    // US-F1-003 should remain in todo
    expect(sprint.storiesByStatus.todo.some((s) => s.id === "US-F1-003")).toBe(true);

    resolveUpdate();
    await waitFor(() => expect(result.current.isIdle).toBe(true));
  });
});

describe("useReorderStories", () => {
  let qc: QueryClient;
  let snapshot: RepositorySnapshot;

  beforeEach(() => {
    vi.resetAllMocks();
    qc = makeQueryClient();
    snapshot = makePrioritySnapshot();
    qc.setQueryData<RepositorySnapshot>(["repository"], snapshot);
  });

  it("optimistically updates priority in cache", async () => {
    let resolveUpdate!: () => void;
    vi.mocked(updateStoryFields).mockReturnValue(
      new Promise<void>((resolve) => {
        resolveUpdate = resolve;
      }),
    );

    const { result } = renderHook(() => useReorderStories(), { wrapper: wrapper(qc) });

    act(() => {
      result.current.mutate({
        orderedIds: ["US-F1-002", "US-F1-001"],
        movedId: "US-F1-002",
        items: snapshot.sprints[0]!.storiesByStatus.todo,
      });
    });

    await act(async () => {
      await Promise.resolve();
    });

    const optimistic = qc.getQueryData<RepositorySnapshot>(["repository"])!;
    expect(optimistic.stories.find((story) => story.id === "US-F1-002")?.priority).toBe(5);
    expect(optimistic.epics[0]?.stories.map((story) => story.id)).toEqual(["US-F1-002", "US-F1-001"]);
    expect(optimistic.sprints[0]?.storiesByStatus.todo.map((story) => story.id)).toEqual(["US-F1-002", "US-F1-001"]);
    expect(updateStoryFields).toHaveBeenCalledWith("US-F1-002", { priority: 5 });

    resolveUpdate();
    await waitFor(() => expect(result.current.isIdle).toBe(true));
  });

  it("normalizes when gap is exhausted", async () => {
    let resolveFirst!: () => void;
    let resolveSecond!: () => void;
    vi.mocked(updateStoryFields)
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockReturnValueOnce(
        new Promise<void>((resolve) => {
          resolveSecond = resolve;
        }),
      );

    const { result } = renderHook(() => useReorderStories(), { wrapper: wrapper(qc) });

    act(() => {
      result.current.mutate({
        orderedIds: ["US-F1-001", "US-F1-003", "US-F1-002"],
        movedId: "US-F1-003",
        items: [
          { id: "US-F1-001", priority: 10 },
          { id: "US-F1-002", priority: 11 },
          { id: "US-F1-003", priority: 30 },
        ],
      });
    });

    await act(async () => {
      await Promise.resolve();
    });

    const optimistic = qc.getQueryData<RepositorySnapshot>(["repository"])!;
    expect(optimistic.stories.find((story) => story.id === "US-F1-003")?.priority).toBe(20);
    expect(optimistic.stories.find((story) => story.id === "US-F1-002")?.priority).toBe(30);
    expect(updateStoryFields).toHaveBeenNthCalledWith(1, "US-F1-003", { priority: 20 });
    expect(updateStoryFields).toHaveBeenNthCalledWith(2, "US-F1-002", { priority: 30 });

    resolveFirst();
    resolveSecond();
    await waitFor(() => expect(result.current.isIdle).toBe(true));
  });
});

describe("useReorderEpics", () => {
  let qc: QueryClient;
  let snapshot: RepositorySnapshot;

  beforeEach(() => {
    vi.resetAllMocks();
    qc = makeQueryClient();
    snapshot = makePrioritySnapshot();
    qc.setQueryData<RepositorySnapshot>(["repository"], snapshot);
  });

  it("optimistically updates priority in cache", async () => {
    let resolveUpdate!: () => void;
    vi.mocked(updateEpicFields).mockReturnValue(
      new Promise<void>((resolve) => {
        resolveUpdate = resolve;
      }),
    );

    const { result } = renderHook(() => useReorderEpics(), { wrapper: wrapper(qc) });

    act(() => {
      result.current.mutate({
        orderedIds: ["EP-F1-02", "EP-F1-01"],
        movedId: "EP-F1-02",
        items: snapshot.epics,
      });
    });

    await act(async () => {
      await Promise.resolve();
    });

    const optimistic = qc.getQueryData<RepositorySnapshot>(["repository"])!;
    expect(optimistic.epics.map((epic) => epic.id)).toEqual(["EP-F1-02", "EP-F1-01"]);
    expect(optimistic.epics.find((epic) => epic.id === "EP-F1-02")?.priority).toBe(5);
    expect(updateEpicFields).toHaveBeenCalledWith("EP-F1-02", { priority: 5 });

    resolveUpdate();
    await waitFor(() => expect(result.current.isIdle).toBe(true));
  });
});
