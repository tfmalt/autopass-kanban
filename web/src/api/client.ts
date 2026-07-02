import type { ConfigResponse, DashboardMetrics, EpicDetail, GitPullResponse, RepositorySnapshot, StoryDetail, TeamMember } from "@shared/generated/api.js";

async function getJson<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`GET ${url} failed: ${res.status}`);
  return (await res.json()) as T;
}

async function sendJson<T = void>(method: "POST" | "PUT" | "PATCH", url: string, body: unknown): Promise<T> {
  const res = await fetch(url, {
    method,
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const data = (await res.json().catch(() => null)) as { error?: unknown } | null;
    const message = typeof data?.error === "string" ? data.error : `${method} ${url} failed: ${res.status}`;
    throw new Error(message);
  }
  return (await res.json().catch(() => undefined)) as T;
}

export const fetchRepository = () => getJson<RepositorySnapshot>("/api/repository");
export const fetchMetrics = () => getJson<DashboardMetrics>("/api/metrics");
export const fetchConfig = () => getJson<ConfigResponse>("/api/config");
export const fetchTeam = () => getJson<TeamMember[]>("/api/team");
export const fetchEpic = (id: string) => getJson<EpicDetail>(`/api/epics/${encodeURIComponent(id)}`);
export const fetchStory = (id: string) => getJson<StoryDetail>(`/api/stories/${encodeURIComponent(id)}`);

export const moveStory = (id: string, status: string, assignee?: string) =>
  sendJson("POST", `/api/stories/${encodeURIComponent(id)}/move`, { status, assignee });
export const planStory = (id: string, sprint: string) =>
  sendJson("POST", `/api/stories/${encodeURIComponent(id)}/plan`, { sprint });
export const createSprint = (input: { headline: string; number?: number; start?: string; end?: string }) =>
  sendJson("POST", "/api/sprints", input);
export const updateSprint = (
  name: string,
  input: { headline: string; goal: string; start: string; end: string; status: string; wipLimit: number | null },
) => sendJson<{ ok: true; data: { name: string; headline: string; sprintPath: string } }>("POST", `/api/sprints/${encodeURIComponent(name)}`, input);

export const updateStory = (id: string, body: string) =>
  sendJson("PUT", `/api/stories/${encodeURIComponent(id)}`, { body });

export const updateStoryFields = (
  id: string,
  fields: {
    assignee?: string;
    sprint?: string;
    status?: string;
    storyPoints?: string | number;
    priority?: number;
  },
) =>
  sendJson("PATCH", `/api/stories/${encodeURIComponent(id)}/fields`, fields);

export const updateEpicFields = (id: string, fields: { priority: number }) =>
  sendJson("PATCH", `/api/epics/${encodeURIComponent(id)}/fields`, fields);

export const updateTaskStatus = (storyId: string, taskId: string, status: string) =>
  sendJson("PATCH", `/api/stories/${encodeURIComponent(storyId)}/tasks/${encodeURIComponent(taskId)}`, { status });

export const gitPull = () => sendJson<GitPullResponse>("POST", "/api/git-pull", {});
