import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { QueryClient } from "@tanstack/react-query";
import type { Epic, RepositorySnapshot, Sprint, Story, StoryStatus } from "@shared/generated/api.js";
import { isBoardStatus, parseAssignees, slugifyHeadline, STORY_STATUSES } from "@shared/domain.js";

export type OptimisticSnapshotContext = { previous?: RepositorySnapshot };

type Rankable = { id: string; priority: number | null };

type MoveStoryVariables = { id: string; status: string; assignee?: string };
type PlanStoryVariables = { id: string; sprint: string };
type ReorderStoriesVariables = {
  orderedIds: string[];
  movedId: string;
  items: Array<Pick<Story, "id" | "priority">>;
};
type ReorderEpicsVariables = {
  orderedIds: string[];
  movedId: string;
  items: Array<Pick<Epic, "id" | "priority">>;
};
type UnplanStoryVariables = { id: string };
type UpdateSprintVariables = {
  name: string;
  headline: string;
  goal: string;
  start: string;
  end: string;
  status: string;
  wipLimit: number | null;
};

type UseOptimisticSnapshotMutationOptions<TData, TVariables> = {
  mutationFn: (vars: TVariables) => Promise<TData>;
  apply: (snapshot: RepositorySnapshot, vars: TVariables) => RepositorySnapshot;
  onSettled?: (args: {
    data: TData | undefined;
    error: Error | null;
    vars: TVariables;
    context: OptimisticSnapshotContext | undefined;
    queryClient: QueryClient;
  }) => void;
};

function mapStoryBuckets(
  storiesByStatus: Record<StoryStatus, Story[]>,
  mapStories: (stories: Story[], status: StoryStatus) => Story[],
): Record<StoryStatus, Story[]> {
  return Object.fromEntries(
    STORY_STATUSES.map((status) => [status, mapStories(storiesByStatus[status], status)]),
  ) as Record<StoryStatus, Story[]>;
}

function patchStoryCollections(snapshot: RepositorySnapshot, id: string, story: Story): RepositorySnapshot {
  return {
    ...snapshot,
    stories: snapshot.stories.map((candidate) => (candidate.id === id ? story : candidate)),
    epics: snapshot.epics.map((epic) => ({
      ...epic,
      stories: epic.stories.map((candidate) => (candidate.id === id ? story : candidate)),
    })),
  };
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

export function patchStoryEverywhere(
  snapshot: RepositorySnapshot,
  id: string,
  patch: Partial<Story>,
): RepositorySnapshot {
  const patchStory = (story: Story): Story => (story.id === id ? { ...story, ...patch } : story);

  return {
    ...snapshot,
    stories: snapshot.stories.map(patchStory),
    epics: snapshot.epics.map((epic) => ({
      ...epic,
      stories: epic.stories.map(patchStory),
    })),
    sprints: snapshot.sprints.map((sprint) => ({
      ...sprint,
      storiesByStatus: mapStoryBuckets(sprint.storiesByStatus, (stories) => stories.map(patchStory)),
    })),
  };
}

export function removeStoryFromSprints(snapshot: RepositorySnapshot, id: string): RepositorySnapshot {
  return {
    ...snapshot,
    sprints: snapshot.sprints.map((sprint) => ({
      ...sprint,
      storiesByStatus: mapStoryBuckets(sprint.storiesByStatus, (stories) =>
        stories.filter((story) => story.id !== id),
      ),
    })),
  };
}

export function moveStoryToBucket(
  snapshot: RepositorySnapshot,
  sprintName: string,
  status: StoryStatus,
  story: Story,
): RepositorySnapshot {
  return {
    ...snapshot,
    sprints: snapshot.sprints.map((sprint) => {
      if (sprint.name !== sprintName) return sprint;

      const storiesByStatus = mapStoryBuckets(sprint.storiesByStatus, (stories) =>
        stories.filter((candidate) => candidate.id !== story.id),
      );
      storiesByStatus[status] = [...storiesByStatus[status], story];

      return { ...sprint, storiesByStatus };
    }),
  };
}

function updateRepositoryStoryPriority(current: RepositorySnapshot, updates: Map<string, number>): RepositorySnapshot {
  let next = current;
  for (const [id, priority] of updates) {
    next = patchStoryEverywhere(next, id, { priority });
  }

  return {
    ...next,
    stories: byPriorityThenId(next.stories),
    epics: next.epics.map((epic) => ({
      ...epic,
      stories: byPriorityThenId(epic.stories),
    })),
    sprints: next.sprints.map((sprint) => ({
      ...sprint,
      storiesByStatus: mapStoryBuckets(sprint.storiesByStatus, (stories) => byPriorityThenId(stories)),
    })),
  };
}

export function applyMoveStorySnapshot(current: RepositorySnapshot, vars: MoveStoryVariables): RepositorySnapshot {
  const story = current.stories.find((candidate) => candidate.id === vars.id);
  if (!story) return current;

  const movedStory: Story = {
    ...story,
    status: vars.status,
    ...(vars.assignee !== undefined
      ? { assignee: vars.assignee, assignees: parseAssignees(vars.assignee) }
      : {}),
  };

  const sprintNames = current.sprints
    .filter((sprint) => STORY_STATUSES.some((status) => sprint.storiesByStatus[status].some((candidate) => candidate.id === vars.id)))
    .map((sprint) => sprint.name);

  let next = patchStoryCollections(current, vars.id, movedStory);
  next = removeStoryFromSprints(next, vars.id);

  if (isBoardStatus(vars.status)) {
    for (const sprintName of sprintNames) {
      next = moveStoryToBucket(next, sprintName, vars.status, movedStory);
    }
  }

  return next;
}

export function applyPlanStorySnapshot(current: RepositorySnapshot, vars: PlanStoryVariables): RepositorySnapshot {
  if (!current.sprints.some((sprint) => sprint.name === vars.sprint)) return current;

  const story = current.stories.find((candidate) => candidate.id === vars.id);
  if (!story) return current;

  const plannedStory: Story = { ...story, status: "todo", sprint: vars.sprint };
  const next = patchStoryCollections(current, vars.id, plannedStory);

  return {
    ...next,
    sprints: next.sprints.map((sprint) =>
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
}

export function applyReorderStoriesSnapshot(current: RepositorySnapshot, vars: ReorderStoriesVariables): RepositorySnapshot {
  const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
  const updatesMap = new Map(updates.map((update) => [update.id, update.priority]));
  if (updatesMap.size === 0) return current;
  return updateRepositoryStoryPriority(current, updatesMap);
}

export function applyReorderEpicsSnapshot(current: RepositorySnapshot, vars: ReorderEpicsVariables): RepositorySnapshot {
  const updates = computePriorityUpdates(vars.orderedIds, vars.movedId, vars.items);
  const updatesMap = new Map(updates.map((update) => [update.id, update.priority]));
  if (updatesMap.size === 0) return current;

  return {
    ...current,
    epics: byPriorityThenId(
      current.epics.map((epic) => {
        const priority = updatesMap.get(epic.id);
        return priority === undefined ? epic : { ...epic, priority };
      }),
    ),
  };
}

export function applyUnplanStorySnapshot(current: RepositorySnapshot, vars: UnplanStoryVariables): RepositorySnapshot {
  const story = current.stories.find((candidate) => candidate.id === vars.id);
  if (!story) return current;

  const unplannedStory: Story = { ...story, status: "ready", sprint: null };
  return removeStoryFromSprints(patchStoryCollections(current, vars.id, unplannedStory), vars.id);
}

export function applyUpdateSprintSnapshot(current: RepositorySnapshot, vars: UpdateSprintVariables): RepositorySnapshot {
  const original = current.sprints.find((sprint) => sprint.name === vars.name);
  const headline = slugifyHeadline(vars.headline);
  const newName = original && headline ? `${original.id}.${headline}` : vars.name;
  const renameStory = (story: Story): Story => (story.sprint === vars.name ? { ...story, sprint: newName } : story);

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
}

export function useOptimisticSnapshotMutation<TData, TVariables>({
  mutationFn,
  apply,
  onSettled,
}: UseOptimisticSnapshotMutationOptions<TData, TVariables>) {
  const queryClient = useQueryClient();

  return useMutation<TData, Error, TVariables, OptimisticSnapshotContext>({
    mutationFn,
    onMutate: async (vars) => {
      await queryClient.cancelQueries({ queryKey: ["repository"] });
      const previous = queryClient.getQueryData<RepositorySnapshot>(["repository"]);

      queryClient.setQueryData<RepositorySnapshot>(["repository"], (current) =>
        current ? apply(current, vars) : current,
      );

      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(["repository"], context.previous);
      }
    },
    onSettled: (data, error, vars, context) => {
      void queryClient.invalidateQueries({ queryKey: ["repository"] });
      onSettled?.({ data, error, vars, context, queryClient });
    },
  });
}
