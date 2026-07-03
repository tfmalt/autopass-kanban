import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { Epic, EpicDetail, Story, StoryDetail } from "@shared/generated/api.js";
import { parseAssignees } from "@shared/domain.js";
import {
  createSprint,
  fetchConfig,
  fetchEpic,
  fetchMetrics,
  fetchReport,
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
import {
  applyMoveStorySnapshot,
  applyPlanStorySnapshot,
  applyReorderEpicsSnapshot,
  applyReorderStoriesSnapshot,
  applyUnplanStorySnapshot,
  applyUpdateSprintSnapshot,
  byPriorityThenId,
  computePriorityUpdates,
  useOptimisticSnapshotMutation,
} from "./optimistic.js";

export { byPriorityThenId, computePriorityUpdates };

export const useRepository = () => useQuery({ queryKey: ["repository"], queryFn: fetchRepository });
export const useMetrics = () => useQuery({ queryKey: ["metrics"], queryFn: fetchMetrics });
export const useReport = () => useQuery({ queryKey: ["report"], queryFn: fetchReport });
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

/** Team roster — TeamMember objects, sourced from .kanban/settings.json or backlog frontmatter. */
export const useTeam = () =>
  useQuery({ queryKey: ["team"], queryFn: fetchTeam, staleTime: 5 * 60 * 1000 });

export function useMoveStory() {
  return useOptimisticSnapshotMutation({
    mutationFn: (vars: { id: string; status: string; assignee?: string }) =>
      moveStory(vars.id, vars.status, vars.assignee),
    apply: applyMoveStorySnapshot,
  });
}

export function usePlanStory() {
  return useOptimisticSnapshotMutation({
    mutationFn: (vars: { id: string; sprint: string }) => planStory(vars.id, vars.sprint),
    apply: applyPlanStorySnapshot,
  });
}

export function useReorderStories() {
  return useOptimisticSnapshotMutation({
    mutationFn: async (vars: {
      orderedIds: string[];
      movedId: string;
      items: Array<Pick<Story, "id" | "priority">>;
    }) => {
      const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
      await Promise.all(updates.map((update) => updateStoryFields(update.id, { priority: update.priority })));
      return updates;
    },
    apply: applyReorderStoriesSnapshot,
  });
}

export function useReorderEpics() {
  return useOptimisticSnapshotMutation({
    mutationFn: async (vars: {
      orderedIds: string[];
      movedId: string;
      items: Array<Pick<Epic, "id" | "priority">>;
    }) => {
      const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
      await Promise.all(updates.map((update) => updateEpicFields(update.id, { priority: update.priority })));
      return updates;
    },
    apply: applyReorderEpicsSnapshot,
  });
}

export function useUnplanStory() {
  return useOptimisticSnapshotMutation({
    mutationFn: (vars: { id: string }) => updateStoryFields(vars.id, { sprint: "", status: "ready" }),
    apply: applyUnplanStorySnapshot,
    onSettled: ({ queryClient, vars }) => {
      void queryClient.invalidateQueries({ queryKey: ["story", vars.id] });
    },
  });
}

export function useCreateSprint() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: createSprint,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["repository"] });
      qc.invalidateQueries({ queryKey: ["report"] });
    },
  });
}

export function useUpdateSprint() {
  return useOptimisticSnapshotMutation({
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
    apply: applyUpdateSprintSnapshot,
  });
}

export function useLiveReload() {
  const qc = useQueryClient();
  useEffect(() => {
    const source = new EventSource("/api/events");
    source.addEventListener("change", () => {
      qc.invalidateQueries({ queryKey: ["repository"] });
      qc.invalidateQueries({ queryKey: ["metrics"] });
      qc.invalidateQueries({ queryKey: ["report"] });
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
      qc.invalidateQueries({ queryKey: ["report"] });
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
      qc.invalidateQueries({ queryKey: ["report"] });
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
      qc.invalidateQueries({ queryKey: ["report"] });
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
