import type { DashboardMetrics, ReportDashboard, RepositorySnapshot, Story } from "@shared/generated/api.js";

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
    epics: [{ id: "EP-F1-01", title: "Platform", phase: "F1", priority: null, planned_start: null, planned_end: null, work_started: null, work_done: null, stories: [done, todo] }],
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

export function report(overrides?: Partial<ReportDashboard>): ReportDashboard {
  return {
    generatedAt: "2026-06-10T10:00:00+0200",
    dailyAvg: 2.67,
    throughputSource: "daily throughput over 3 observed workdays",
    hoursPerPoint: 7 / 2.67,
    remainingPoints: 8,
    progress: { donePoints: 5, totalPoints: 13, doneStories: 1, totalStories: 2 },
    forecast: metrics().forecast,
    estimates: [
      { storyId: "US-F1-001", estHours: null, estStart: "2026-06-01", estEnd: "2026-06-03" },
      { storyId: "US-F1-002", estHours: 21, estStart: "2026-06-10", estEnd: "2026-06-15" },
    ],
    wbsRows: [
      { kind: "phase", wbs: "1", id: "F1", title: "Phase 1 - Etablering (Establishment)", milestone: "MP1 - Foundation", period: "Q2 2026", priority: "Critical", status: "", points: 13, estHours: null, startDate: "2026-06-01", endDate: "2026-06-15", notes: "" },
      { kind: "epic", wbs: "1.1", id: "EP-F1-01", title: "Platform", milestone: "MP1 - Foundation", period: "Q2 2026", priority: "Critical", status: "", points: 13, estHours: null, startDate: "2026-06-01", endDate: "2026-06-15", notes: "" },
      { kind: "story", wbs: "1.1.1", id: "US-F1-001", title: "Done story", milestone: "MP1 - Foundation", period: "Q2 2026", priority: "Critical", status: "DONE", points: 5, estHours: 13.1, startDate: "2026-06-01", endDate: "2026-06-03", notes: "Sprint S000.start" },
      { kind: "story", wbs: "1.1.2", id: "US-F1-002", title: "Todo story", milestone: "MP1 - Foundation", period: "Q2 2026", priority: "Critical", status: "TODO", points: 8, estHours: 21, startDate: "2026-06-10", endDate: "2026-06-15", notes: "Sprint S001.next; Assignee Test User <test@example.com>" },
    ],
    phaseRows: [
      { phase: "F1", title: "Phase 1 - Etablering (Establishment)", period: "Q2 2026", milestone: "MP1 - Foundation", epics: 1, stories: 2, total: 13, done: 5, wip: 0, remaining: 8 },
    ],
    sprintRows: [
      { name: "S000.start", startDate: "2026-06-01", endDate: "2026-06-14", plannedPoints: 5, deliveredPoints: 5, rate: 2.67, remaining: 8, status: "closed" },
      { name: "S002.projected", startDate: "2026-06-15", endDate: "2026-06-28", plannedPoints: 27, deliveredPoints: 8, rate: 2.7, remaining: 0, status: "projected (daily throughput over 3 observed workdays)" },
    ],
    ...overrides,
  };
}
