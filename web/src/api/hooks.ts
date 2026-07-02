import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { Epic, EpicDetail, RepositorySnapshot, Sprint, Story, StoryDetail, StoryStatus } from "@shared/types.js";
import { isBoardStatus, parseAssignees, STORY_STATUSES } from "@shared/types.js";
import {
  createSprint,
  fetchConfig,
  fetchEpic,
  fetchMetrics,
  fetchRepository,
  fetchStory,
  fetchTeam,
  gitPull,
  moveStory,
  planStory,
  updateEpicFields,
  updateSprint,
  updateStory,
  updateStoryFields,
  updateTaskStatus,
} from "./client.js";

type Rankable = { id: string; priority: number | null };

export const useRepository = () => useQuery({ queryKey: ["repository"], queryFn: fetchRepository });
export const useMetrics = () => useQuery({ queryKey: ["metrics"], queryFn: fetchMetrics });
export const useConfig = () => useQuery({ queryKey: ["config"], queryFn: fetchConfig, staleTime: Infinity });

export function useGitPull() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: gitPull,
    onSuccess: (data) => {
      if (data.ok) {
        void queryClient.invalidateQueries();
      }
    },
  });
}

export function byPriorityThenId<T extends { priority: number | null; id: string }>(items: T[]): T[] {
  return [...items].sort((a, b) => {
    const pa = a.priority ?? Infinity;
    const pb = b.priority ?? Infinity;
    if (pa !== pb) return pa - pb;
    return a.id.localeCompare(b.id);
  });
}

export function computePriorityUpdates<T extends Rankable>(
  orderedIds: string[],
  movedId: string,
  items: T[],
): Array<{ id: string; priority: number }> {
  const itemById = new Map(items.map((item) => [item.id, item]));
  const orderedItems = orderedIds
    .map((id) => itemById.get(id))
    .filter((item): item is T => item !== undefined);
  if (orderedItems.length === 0) return [];

  const normalize = () =>
    orderedItems
      .map((item, index) => ({ id: item.id, priority: (index + 1) * 10 }))
      .filter((update) => itemById.get(update.id)?.priority !== update.priority);

  if (orderedItems.some((item) => item.priority === null)) return normalize();

  if (orderedItems.length === 1) {
    return orderedItems[0]!.priority === 10 ? [] : [{ id: orderedItems[0]!.id, priority: 10 }];
  }

  const movedIndex = orderedIds.indexOf(movedId);
  if (movedIndex === -1) return [];

  const left = movedIndex > 0 ? itemById.get(orderedIds[movedIndex - 1]!) ?? null : null;
  const right = movedIndex < orderedIds.length - 1 ? itemById.get(orderedIds[movedIndex + 1]!) ?? null : null;

  let priority: number | null = null;

  if (!left && !right) {
    priority = 10;
  } else if (!left && right) {
    priority = Math.floor((right.priority ?? 0) / 2);
    if (priority === right.priority) return normalize();
  } else if (left && !right) {
    priority = (left.priority ?? 0) + 10;
  } else if (left && right) {
    priority = Math.floor(((left.priority ?? 0) + (right.priority ?? 0)) / 2);
    if (priority === left.priority || priority === right.priority) return normalize();
  }

  if (priority === null) return [];
  return itemById.get(movedId)?.priority === priority ? [] : [{ id: movedId, priority }];
}

/** Team roster — TeamMember objects, sourced from .kanban/settings.json or backlog frontmatter. */
export const useTeam = () =>
  useQuery({ queryKey: ["team"], queryFn: fetchTeam, staleTime: 5 * 60 * 1000 });

export function useMoveStory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: string; status: string; assignee?: string }) =>
      moveStory(vars.id, vars.status, vars.assignee),
    onMutate: async (vars) => {
      await qc.cancelQueries({ queryKey: ["repository"] });
      const previous = qc.getQueryData<RepositorySnapshot>(["repository"]);
      qc.setQueryData<RepositorySnapshot>(["repository"], (current) => {
        if (!current) return current;
        const story = current.stories.find((s) => s.id === vars.id);
        if (!story) return current;
        const movedStory: Story = {
          ...story,
          status: vars.status,
          ...(vars.assignee !== undefined
            ? { assignee: vars.assignee, assignees: parseAssignees(vars.assignee) }
            : {}),
        };
        return {
          ...current,
          stories: current.stories.map((s) => (s.id === vars.id ? movedStory : s)),
          epics: current.epics.map((epic) => ({
            ...epic,
            stories: epic.stories.map((s) => (s.id === vars.id ? movedStory : s)),
          })),
          sprints: current.sprints.map((sprint) => {
            const isInSprint = STORY_STATUSES.some((st) =>
              sprint.storiesByStatus[st].some((s) => s.id === vars.id),
            );
            if (!isInSprint) return sprint;
            // Remove from every bucket, then insert into the new one.
            const newStoriesByStatus = Object.fromEntries(
              STORY_STATUSES.map((st) => [st, sprint.storiesByStatus[st].filter((s) => s.id !== vars.id)]),
            ) as Record<StoryStatus, Story[]>;
            if (isBoardStatus(vars.status)) {
              newStoriesByStatus[vars.status] = [
                ...newStoriesByStatus[vars.status],
                movedStory,
              ];
            }
            return { ...sprint, storiesByStatus: newStoriesByStatus };
          }),
        };
      });
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(["repository"], context.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["repository"] }),
  });
}

export function usePlanStory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: string; sprint: string }) => planStory(vars.id, vars.sprint),
    onMutate: async (vars) => {
      await qc.cancelQueries({ queryKey: ["repository"] });
      const previous = qc.getQueryData<RepositorySnapshot>(["repository"]);
      qc.setQueryData<RepositorySnapshot>(["repository"], (current) => {
        if (!current || !current.sprints.some((sprint) => sprint.name === vars.sprint)) return current;
        const story = current.stories.find((candidate) => candidate.id === vars.id);
        if (!story) return current;

        const plannedStory = { ...story, status: "todo", sprint: vars.sprint };
        return {
          ...current,
          stories: current.stories.map((candidate) =>
            candidate.id === vars.id ? plannedStory : candidate,
          ),
          epics: current.epics.map((epic) => ({
            ...epic,
            stories: epic.stories.map((candidate) =>
              candidate.id === vars.id ? plannedStory : candidate,
            ),
          })),
          sprints: current.sprints.map((sprint) =>
            sprint.name === vars.sprint
              ? {
                  ...sprint,
                  storiesByStatus: {
                    ...sprint.storiesByStatus,
                    todo: [...sprint.storiesByStatus.todo.filter((candidate) => candidate.id !== vars.id), plannedStory],
                  },
                }
              : sprint,
          ),
        };
      });
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(["repository"], context.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["repository"] }),
  });
}

function updateRepositoryStoryPriority(current: RepositorySnapshot, updates: Map<string, number>): RepositorySnapshot {
  const updateStoryPriority = (story: Story): Story => {
    const priority = updates.get(story.id);
    return priority === undefined ? story : { ...story, priority };
  };

  return {
    ...current,
    stories: byPriorityThenId(current.stories.map(updateStoryPriority)),
    epics: current.epics.map((epic) => ({
      ...epic,
      stories: byPriorityThenId(epic.stories.map(updateStoryPriority)),
    })),
    sprints: current.sprints.map((sprint) => ({
      ...sprint,
      storiesByStatus: Object.fromEntries(
        STORY_STATUSES.map((status) => [status, byPriorityThenId(sprint.storiesByStatus[status].map(updateStoryPriority))]),
      ) as Record<StoryStatus, Story[]>,
    })),
  };
}

export function useReorderStories() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (vars: {
      orderedIds: string[];
      movedId: string;
      items: Array<Pick<Story, "id" | "priority">>;
    }) => {
      const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
      await Promise.all(updates.map((update) => updateStoryFields(update.id, { priority: update.priority })));
      return updates;
    },
    onMutate: async (vars) => {
      await qc.cancelQueries({ queryKey: ["repository"] });
      const previous = qc.getQueryData<RepositorySnapshot>(["repository"]);
      const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
      const updatesMap = new Map(updates.map((update) => [update.id, update.priority]));

      qc.setQueryData<RepositorySnapshot>(["repository"], (current) => {
        if (!current || updatesMap.size === 0) return current;
        return updateRepositoryStoryPriority(current, updatesMap);
      });

      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(["repository"], context.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["repository"] }),
  });
}

export function useReorderEpics() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (vars: {
      orderedIds: string[];
      movedId: string;
      items: Array<Pick<Epic, "id" | "priority">>;
    }) => {
      const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
      await Promise.all(updates.map((update) => updateEpicFields(update.id, { priority: update.priority })));
      return updates;
    },
    onMutate: async (vars) => {
      await qc.cancelQueries({ queryKey: ["repository"] });
      const previous = qc.getQueryData<RepositorySnapshot>(["repository"]);
      const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
      const updatesMap = new Map(updates.map((update) => [update.id, update.priority]));

      qc.setQueryData<RepositorySnapshot>(["repository"], (current) => {
        if (!current || updatesMap.size === 0) return current;
        return {
          ...current,
          epics: byPriorityThenId(
            current.epics.map((epic) => {
              const priority = updatesMap.get(epic.id);
              return priority === undefined ? epic : { ...epic, priority };
            }),
          ),
        };
      });

      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(["repository"], context.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["repository"] }),
  });
}

export function useUnplanStory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: string }) => updateStoryFields(vars.id, { sprint: "", status: "ready" }),
    onMutate: async (vars) => {
      await qc.cancelQueries({ queryKey: ["repository"] });
      const previous = qc.getQueryData<RepositorySnapshot>(["repository"]);
      qc.setQueryData<RepositorySnapshot>(["repository"], (current) => {
        if (!current) return current;
        const story = current.stories.find((candidate) => candidate.id === vars.id);
        if (!story) return current;

        const unplannedStory: Story = { ...story, status: "ready", sprint: null };
        return {
          ...current,
          stories: current.stories.map((candidate) =>
            candidate.id === vars.id ? unplannedStory : candidate,
          ),
          epics: current.epics.map((epic) => ({
            ...epic,
            stories: epic.stories.map((candidate) =>
              candidate.id === vars.id ? unplannedStory : candidate,
            ),
          })),
          sprints: current.sprints.map((sprint) => ({
            ...sprint,
            storiesByStatus: Object.fromEntries(
              STORY_STATUSES.map((status) => [
                status,
                sprint.storiesByStatus[status].filter((candidate) => candidate.id !== vars.id),
              ]),
            ) as Record<StoryStatus, Story[]>,
          })),
        };
      });
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(["repository"], context.previous);
    },
    onSettled: (_data, _error, vars) => {
      qc.invalidateQueries({ queryKey: ["repository"] });
      qc.invalidateQueries({ queryKey: ["story", vars.id] });
    },
  });
}

export function useCreateSprint() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: createSprint,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["repository"] }),
  });
}

function slugifyHeadline(value: string): string {
  let slug = "";
  let lastWasDash = false;
  for (const ch of value.trim()) {
    const normalized = ch.toLowerCase();
    if (/^[a-z0-9]$/.test(normalized)) {
      slug += normalized;
      lastWasDash = false;
    } else if (!lastWasDash && slug.length > 0) {
      slug += "-";
      lastWasDash = true;
    }
  }
  return slug.replace(/^-+|-+$/g, "");
}

export function useUpdateSprint() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: {
      name: string;
      headline: string;
      goal: string;
      start: string;
      end: string;
      status: string;
      wipLimit: number | null;
    }) => updateSprint(vars.name, {
      headline: vars.headline,
      goal: vars.goal,
      start: vars.start,
      end: vars.end,
      status: vars.status,
      wipLimit: vars.wipLimit,
    }),
    onMutate: async (vars) => {
      await qc.cancelQueries({ queryKey: ["repository"] });
      const previous = qc.getQueryData<RepositorySnapshot>(["repository"]);
      qc.setQueryData<RepositorySnapshot>(["repository"], (current) => {
        if (!current) return current;
        const original = current.sprints.find((sprint) => sprint.name === vars.name);
        const headline = slugifyHeadline(vars.headline);
        const newName = original && headline ? `${original.id}.${headline}` : vars.name;
        const renameStory = (story: Story): Story => story.sprint === vars.name ? { ...story, sprint: newName } : story;
        return {
          ...current,
          stories: current.stories.map(renameStory),
          epics: current.epics.map((epic) => ({
            ...epic,
            stories: epic.stories.map(renameStory),
          })),
          sprints: current.sprints.map((sprint): Sprint => sprint.name === vars.name
            ? {
                ...sprint,
                name: newName,
                headline,
                goal: vars.goal,
                startDate: vars.start,
                endDate: vars.end,
                status: vars.status,
                wipLimit: vars.wipLimit,
              }
            : sprint),
        };
      });
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(["repository"], context.previous);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["repository"] }),
  });
}

export function useLiveReload() {
  const qc = useQueryClient();
  useEffect(() => {
    const source = new EventSource("/api/events");
    source.addEventListener("change", () => {
      qc.invalidateQueries({ queryKey: ["repository"] });
      qc.invalidateQueries({ queryKey: ["metrics"] });
    });
    return () => source.close();
  }, [qc]);
}

/** Fetch a single story with its full markdown body. Pass null to disable. */
export function useStory(id: string | null) {
  return useQuery({
    queryKey: ["story", id],
    queryFn: () => fetchStory(id!),
    enabled: id !== null,
  });
}

export function useEpic(id: string | null) {
  return useQuery<EpicDetail>({
    queryKey: ["epic", id],
    queryFn: () => fetchEpic(id!),
    enabled: id !== null,
  });
}

/** Save updated story body prose; invalidates the story and repository queries on success. */
export function useUpdateStory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: string; body: string }) => updateStory(vars.id, vars.body),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: ["story", vars.id] });
      qc.invalidateQueries({ queryKey: ["repository"] });
    },
  });
}

/**
 * Update story metadata fields (assignee and/or sprint).
 * Sprint changes re-plan the story into todo in the new sprint.
 */
export function useUpdateStoryFields() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: {
      id: string;
      fields: {
        assignee?: string;
        sprint?: string;
        status?: string;
        storyPoints?: string | number;
        priority?: number;
      };
    }) =>
      updateStoryFields(vars.id, vars.fields),
    onSuccess: (_data, vars) => {
      qc.setQueryData<StoryDetail | undefined>(["story", vars.id], (current) => {
        if (!current) return current;
        const storyPoints =
          vars.fields.storyPoints !== undefined
            ? parseStoryPoints(vars.fields.storyPoints)
            : current.storyPoints;
        const status =
          vars.fields.status !== undefined
            ? vars.fields.status
            : vars.fields.sprint !== undefined && vars.fields.status === undefined
              ? "todo"
              : current.status;
        return {
          ...current,
          ...(vars.fields.assignee !== undefined
            ? { assignee: vars.fields.assignee, assignees: parseAssignees(vars.fields.assignee) }
            : {}),
          ...(vars.fields.sprint !== undefined
            ? { sprint: vars.fields.sprint }
            : {}),
          ...(vars.fields.status !== undefined || vars.fields.sprint !== undefined
            ? { status }
            : {}),
          ...(vars.fields.storyPoints !== undefined ? { storyPoints } : {}),
          ...(vars.fields.priority !== undefined ? { priority: vars.fields.priority } : {}),
        };
      });
      qc.invalidateQueries({ queryKey: ["story", vars.id] });
      qc.invalidateQueries({ queryKey: ["repository"] });
    },
  });
}

export function useUpdateTaskStatus() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: { storyId: string; taskId: string; status: string }) =>
      updateTaskStatus(vars.storyId, vars.taskId, vars.status),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: ["story", vars.storyId] });
      qc.invalidateQueries({ queryKey: ["repository"] });
    },
  });
}

function parseStoryPoints(value: string | number): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  const trimmed = value.trim();
  if (trimmed === "") return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}
