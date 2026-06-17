import type { ConfigResponse, DashboardMetrics, EpicDetail, RepositorySnapshot, StoryDetail } from "@shared/types.js";

async function getJson<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`GET ${url} failed: ${res.status}`);
  return (await res.json()) as T;
}

async function postJson<T = void>(url: string, body: unknown): Promise<T> {
  const res = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const data = (await res.json().catch(() => null)) as { error?: unknown } | null;
    const message = typeof data?.error === "string" ? data.error : `POST ${url} failed: ${res.status}`;
    throw new Error(message);
  }
  return (await res.json().catch(() => undefined)) as T;
}

async function putJson(url: string, body: unknown): Promise<void> {
  const res = await fetch(url, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const data = (await res.json().catch(() => null)) as { error?: unknown } | null;
    const message = typeof data?.error === "string" ? data.error : `PUT ${url} failed: ${res.status}`;
    throw new Error(message);
  }
}

async function patchJson(url: string, body: unknown): Promise<void> {
  const res = await fetch(url, {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const data = (await res.json().catch(() => null)) as { error?: unknown } | null;
    const message = typeof data?.error === "string" ? data.error : `PATCH ${url} failed: ${res.status}`;
    throw new Error(message);
  }
}

export const fetchRepository = () => getJson<RepositorySnapshot>("/api/repository");
export const fetchMetrics = () => getJson<DashboardMetrics>("/api/metrics");
export const fetchConfig = () => getJson<ConfigResponse>("/api/config");
export const fetchTeam = () => getJson<string[]>("/api/team");
export const fetchEpic = (id: string) => getJson<EpicDetail>(`/api/epics/${encodeURIComponent(id)}`);
export const fetchStory = (id: string) => getJson<StoryDetail>(`/api/stories/${encodeURIComponent(id)}`);

export const moveStory = (id: string, status: string, assignee?: string) =>
  postJson(`/api/stories/${encodeURIComponent(id)}/move`, { status, assignee });
export const planStory = (id: string, sprint: string) =>
  postJson(`/api/stories/${encodeURIComponent(id)}/plan`, { sprint });
export const createSprint = (input: { headline: string; number?: number; start?: string; end?: string }) =>
  postJson("/api/sprints", input);
export const updateSprint = (
  name: string,
  input: { headline: string; goal: string; start: string; end: string; status: string; wipLimit: number | null },
) => postJson<{ ok: true; data: { name: string; headline: string; sprintPath: string } }>(`/api/sprints/${encodeURIComponent(name)}`, input);

export const updateStory = (id: string, body: string) =>
  putJson(`/api/stories/${encodeURIComponent(id)}`, { body });

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
  patchJson(`/api/stories/${encodeURIComponent(id)}/fields`, fields);

export const updateEpicFields = (id: string, fields: { priority: number }) =>
  patchJson(`/api/epics/${encodeURIComponent(id)}/fields`, fields);

export const updateTaskStatus = (storyId: string, taskId: string, status: string) =>
  patchJson(`/api/stories/${encodeURIComponent(storyId)}/tasks/${encodeURIComponent(taskId)}`, { status });
