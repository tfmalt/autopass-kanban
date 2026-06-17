import type { DashboardMetrics, RepositorySnapshot, Story } from "@shared/types.js";

export function story(input: Partial<Story> & Pick<Story, "id" | "title" | "status" | "storyPoints">): Story {
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

export function repository(overrides?: Partial<RepositorySnapshot>): RepositorySnapshot {
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
        storiesByStatus: { todo: [], "in-progress": [], "ready-for-qa": [], done: [done], blocked: [] },
      },
    ],
    progress: {
      donePoints: 5,
      totalPoints: 13,
      doneStories: 1,
      totalStories: 2,
      phases: [{ phase: "F1", donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2 }],
    },
    ...overrides,
  };
}

export function metrics(overrides?: Partial<DashboardMetrics>): DashboardMetrics {
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
    ...overrides,
  };
}
