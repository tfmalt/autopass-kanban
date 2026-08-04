import { useEffect, useMemo, useState } from "react";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { TeamMember } from "@shared/generated/api.js";
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

/** Query keys that a source change invalidates. */
const AGGREGATE_QUERY_KEYS = [["repository"], ["metrics"], ["report"], ["team"]] as const;

// `keepPreviousData` keeps the last good render on screen during a background
// refetch instead of unmounting the view back to a loading state, which is what
// produced a full-page layout shift on every live-reload event.
export const useRepository = () =>
  useQuery({ queryKey: ["repository"], queryFn: fetchRepository, placeholderData: keepPreviousData });
export const useMetrics = () =>
  useQuery({ queryKey: ["metrics"], queryFn: fetchMetrics, placeholderData: keepPreviousData });
export const useReport = () =>
  useQuery({ queryKey: ["report"], queryFn: fetchReport, placeholderData: keepPreviousData });
// Configuration only changes when the user edits `.kanban/settings.json`, which
// the watcher reports; there is no value in a time-based refetch.
export const useConfig = () => useQuery({ queryKey: ["config"], queryFn: fetchConfig, staleTime: Infinity });

export function useGitPull() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: gitPull,
    onSuccess: (data) => {
      if (data.ok) {
        // Invalidate the aggregates explicitly. An unfiltered
        // `invalidateQueries()` also discarded `["config"]` and every
        // `["story", id]` entry, forcing an open story modal to refetch and
        // the header to flicker for no reason.
        for (const queryKey of AGGREGATE_QUERY_KEYS) {
          void queryClient.invalidateQueries({ queryKey });
        }
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

/** How often to poll the aggregates while the live-reload stream is down. */
const LIVE_RELOAD_FALLBACK_POLL_MS = 30_000;

export type LiveReloadState = {
  /** False while the change stream is unavailable and polling has taken over. */
  connected: boolean;
};

/**
 * Subscribe to server-sent source-change events.
 *
 * The server coalesces a burst of filesystem changes into one event carrying a
 * monotonic generation as the SSE event id, and replays a `resync` event when a
 * reconnecting client is behind. `EventSource` sends `Last-Event-ID`
 * automatically, so no id bookkeeping is needed here.
 *
 * Losing the stream must never leave the UI permanently stale, so an error
 * switches on a polling fallback and surfaces the degraded state to the caller.
 */
export function useLiveReload(): LiveReloadState {
  const qc = useQueryClient();
  const [connected, setConnected] = useState(true);

  useEffect(() => {
    const source = new EventSource("/api/events");
    let frame: number | null = null;
    let poll: ReturnType<typeof setInterval> | null = null;

    const invalidateAggregates = () => {
      for (const queryKey of AGGREGATE_QUERY_KEYS) {
        void qc.invalidateQueries({ queryKey });
      }
    };

    // Client-side backstop: the server already coalesces bursts, but a resync
    // arriving alongside a live change must still cost only one refetch.
    const scheduleInvalidate = () => {
      if (frame !== null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        invalidateAggregates();
      });
    };

    const stopPolling = () => {
      if (poll !== null) {
        clearInterval(poll);
        poll = null;
      }
    };

    const startPolling = () => {
      if (poll !== null) return;
      poll = setInterval(invalidateAggregates, LIVE_RELOAD_FALLBACK_POLL_MS);
    };

    const onOpen = () => {
      setConnected(true);
      stopPolling();
    };

    const onChange = () => {
      setConnected(true);
      stopPolling();
      scheduleInvalidate();
    };

    // Reached when the connection drops, and when the server refuses the
    // subscription outright (`503`, subscriber cap). In both cases the client
    // would otherwise sit silently on stale data forever.
    const onError = () => {
      setConnected(false);
      startPolling();
      // Whatever happened, assume something changed while disconnected.
      scheduleInvalidate();
    };

    source.addEventListener("open", onOpen);
    source.addEventListener("change", onChange);
    source.addEventListener("error", onError);

    return () => {
      if (frame !== null) cancelAnimationFrame(frame);
      stopPolling();
      source.removeEventListener("open", onOpen);
      source.removeEventListener("change", onChange);
      source.removeEventListener("error", onError);
      source.close();
    };
  }, [qc]);

  return { connected };
}

/**
 * Resolve the team roster into an email-keyed lookup **once**.
 *
 * Calling `useTeam` inside a card component created one `QueryObserver`
 * subscription and one `Map` allocation per rendered card, and every team-cache
 * update notified all of them. Resolve at the board or column level and pass the
 * result down.
 */
export function useAssigneeMap(): Map<string, TeamMember> {
  const team = useTeam();
  return useMemo(() => {
    const map = new Map<string, TeamMember>();
    for (const member of team.data ?? []) {
      map.set(member.email, member);
    }
    return map;
  }, [team.data]);
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
