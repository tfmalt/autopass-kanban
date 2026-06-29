export type StoryStatus =
  | "planned"
  | "todo"
  | "in-progress"
  | "ready-for-qa"
  | "done"
  | "blocked";

export const STORY_STATUSES: StoryStatus[] = [
  "planned",
  "todo",
  "in-progress",
  "ready-for-qa",
  "done",
  "blocked",
];

export type StoryLifecycleStatus =
  | "draft"
  | "ready"
  | "planned"
  | "todo"
  | "in-progress"
  | "ready-for-qa"
  | "done"
  | "blocked"
  | "dropped";

export const STORY_LIFECYCLE_STATUSES: StoryLifecycleStatus[] = [
  "draft",
  "ready",
  "planned",
  "todo",
  "in-progress",
  "ready-for-qa",
  "done",
  "blocked",
  "dropped",
];

export function normalizeStatus(value: string): string {
  return value.toLowerCase().trim();
}

export function isBoardStatus(value: string): value is StoryStatus {
  return (STORY_STATUSES as string[]).includes(value);
}

export type SprintStatus = "planned" | "active" | "closed" | "cancelled";

export const SPRINT_STATUSES: SprintStatus[] = ["planned", "active", "closed", "cancelled"];

export type TaskStatus = "todo" | "in-progress" | "blocked" | "done";

export const TASK_STATUSES: TaskStatus[] = ["todo", "in-progress", "blocked", "done"];

export interface Task {
  id: string;
  title: string;
  status: string;
  tags: string[];
  description: string;
}

export interface TaskSummary {
  todo: number;
  inProgress: number;
  readyForQa: number;
  done: number;
  blocked: number;
  total: number;
}

export interface Story {
  id: string;
  title: string;
  status: string;
  phase: string | null;
  epic: string | null;
  sprint: string | null;
  priority: number | null;
  storyPoints: number | null;
  assignee: string | null;
  assignees: string[];
  workStarted: string | null;
  workDone: string | null;
  activated: string | null;
  created: string | null;
  updated: string | null;
  relativePath: string;
  tasks: Task[];
  taskSummary: TaskSummary;
  frontmatter: Record<string, string>;
}

export interface Sprint {
  name: string;
  id: string;
  headline: string;
  goal: string | null;
  startDate: string | null;
  endDate: string | null;
  status: string | null;
  wipLimit: number | null;
  storiesByStatus: Record<StoryStatus, Story[]>;
}

export interface Epic {
  id: string;
  title: string;
  phase: string;
  priority: number | null;
  stories: Story[];
}

export interface PhaseSummary {
  phase: string;
  donePoints: number;
  totalPoints: number;
  doneStories: number;
  totalStories: number;
}

export interface ProjectProgress {
  donePoints: number;
  totalPoints: number;
  doneStories: number;
  totalStories: number;
  phases: PhaseSummary[];
}

export interface RepositorySnapshot {
  stories: Story[];
  epics: Epic[];
  sprints: Sprint[];
  progress: ProjectProgress;
}

export interface BurndownPoint {
  date: string;
  remaining: number;
  ideal: number;
}
export interface BurnupPoint {
  date: string;
  completed: number;
  scope: number;
}
export interface LeadTimePoint {
  storyId: string;
  date: string;
  days: number;
  rollingAvg: number;
}
export interface VelocityPoint {
  sprint: string;
  points: number;
  forecast: boolean;
}
export interface Forecast {
  generatedAt: string;
  remainingPoints: number;
  sprintDurationWeeks: number;
  projectionStartDate: string;
  throughput: {
    samples: number[];
    average: number;
    median: number;
    observedDayCount: number;
  };
  completion: {
    p50Days: number | null;
    p80Days: number | null;
    p90Days: number | null;
    p50Date: string | null;
    p80Date: string | null;
    p90Date: string | null;
  };
  confidence: string;
}
export interface DashboardMetrics {
  burndown: BurndownPoint[];
  burnup: BurnupPoint[];
  leadTime: LeadTimePoint[];
  velocity: VelocityPoint[];
  forecast: Forecast;
  progress: ProjectProgress;
}

/** Full story including markdown body — returned by GET /api/stories/:id */
export interface StoryDetail extends Story {
  body: string;
}

/** Full epic including markdown body — returned by GET /api/epics/:id */
export interface EpicDetail extends Epic {
  body: string;
}

export interface TeamMember {
  name: string;
  email: string;
  label: string;
  avatarUrl?: string;
}

export interface ConfigResponse {
  port: number;
  host: string;
  style: string;
  version: string;
  branch: string;
  storyPoints: {
    allowedValues: string[];
    aliases: Record<string, string>;
  };
}

const ASSIGNEE_PLACEHOLDER = /^Name <email@example\.com>$/i;

export function parseAssignees(value: string | string[] | null | undefined): string[] {
  const values = Array.isArray(value) ? value : typeof value === "string" ? value.split(",") : [];
  return values
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)
    .filter((entry) => entry !== "~")
    .filter((entry) => entry.toUpperCase() !== "TBD")
    .filter((entry) => !ASSIGNEE_PLACEHOLDER.test(entry));
}

export function abbreviateAssignee(value: string): string {
  const name = value.split("<", 1)[0]?.trim() ?? "";
  const firstWord = name.split(/\s+/, 1)[0] ?? "";
  const short = firstWord || name || value.trim();
  return short.slice(0, 6);
}
