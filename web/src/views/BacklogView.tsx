import { Suspense, lazy, useMemo, useRef, useState } from "react";
import {
  DndContext,
  DragOverlay,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, sortableKeyboardCoordinates, verticalListSortingStrategy } from "@dnd-kit/sortable";
import type { Story } from "@shared/generated/api.js";
import { STORY_STATUSES } from "@shared/domain.js";
import { byPriorityThenId, usePlanStory, useReorderEpics, useReorderStories, useRepository, useUnplanStory } from "../api/hooks.js";
import { BacklogStoryCard, BacklogStoryOverlay } from "../components/backlog/BacklogStoryCard.js";
import { BacklogDropZone, SprintDropZone } from "../components/backlog/DropZones.js";
import { BACKLOG_DROP_ID, NO_EPIC_GROUP_ID, SPRINT_DROP_ID } from "../components/backlog/constants.js";
import { EpicDragOverlay, SortableEpicSection } from "../components/backlog/SortableEpicSection.js";
import type { StoryStatusOption } from "../components/StoryModal.js";

// Deferred for the same reason as on the board: the modal and its markdown
// sanitizer are only needed once a story is actually opened.
const StoryModal = lazy(async () => {
  const module = await import("../components/StoryModal.js");
  return { default: module.StoryModal };
});

const BACKLOG_STORY_STATUS_OPTIONS: StoryStatusOption[] = [
  { value: "draft", label: "draft" },
  { value: "ready", label: "ready" },
  { value: "planned", label: "planned" },
  { value: "todo", label: "todo" },
  { value: "in-progress", label: "in-progress" },
  { value: "ready-for-qa", label: "ready-for-qa" },
  { value: "done", label: "done" },
  { value: "blocked", label: "blocked" },
];

export function BacklogView() {
  const repo = useRepository();
  const plan = usePlanStory();
  const unplan = useUnplanStory();
  const reorderStories = useReorderStories();
  const reorderEpics = useReorderEpics();
  const [sprint, setSprint] = useState<string>("");
  const [search, setSearch] = useState("");
  const [open, setOpen] = useState<Story | null>(null);
  const [collapsedEpics, setCollapsedEpics] = useState<Set<string>>(new Set());
  const [expandedEpicDescriptions, setExpandedEpicDescriptions] = useState<Set<string>>(new Set());
  const [activeId, setActiveId] = useState<string | null>(null);
  const dragActivatedRef = useRef(false);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const backlogReorderDisabled = search.trim() !== "";
  const sprints = repo.data?.sprints ?? [];
  const targetSprint = sprint || sprints.find((candidate) => candidate.status === "planned")?.name || sprints.at(-1)?.name || "";
  const visibleSprint = sprints.find((candidate) => candidate.name === targetSprint);

  const unplanned = useMemo(() => {
    if (!repo.data) return [];
    return repo.data.stories.filter(
      (story) => !story.sprint && story.status !== "done" && (search === "" || `${story.id} ${story.title}`.toLowerCase().includes(search.toLowerCase())),
    );
  }, [repo.data, search]);

  const epicOrder = useMemo(() => {
    return byPriorityThenId(repo.data?.epics ?? []);
  }, [repo.data?.epics]);

  const storiesByEpic = useMemo(() => {
    const map = new Map<string, Story[]>();
    for (const story of unplanned) {
      const key = story.epic ?? NO_EPIC_GROUP_ID;
      map.set(key, [...(map.get(key) ?? []), story]);
    }
    for (const [key, stories] of map.entries()) {
      map.set(key, byPriorityThenId(stories));
    }
    return map;
  }, [unplanned]);

  const noEpicStories = storiesByEpic.get(NO_EPIC_GROUP_ID) ?? [];

  const targetStories = useMemo(() => {
    if (!visibleSprint) return [];
    return byPriorityThenId(STORY_STATUSES.flatMap((status) => visibleSprint.storiesByStatus[status] ?? []));
  }, [visibleSprint]);

  const activeStory = useMemo(() => repo.data?.stories.find((story) => story.id === activeId) ?? null, [activeId, repo.data?.stories]);
  const activeEpic = useMemo(() => repo.data?.epics.find((epic) => epic.id === activeId) ?? null, [activeId, repo.data?.epics]);

  const planStory = (storyId: string) => {
    if (!targetSprint) return;
    plan.mutate({ id: storyId, sprint: targetSprint });
  };

  const unplanStory = (storyId: string) => {
    unplan.mutate({ id: storyId });
  };

  const toggleEpic = (epicId: string) => {
    setCollapsedEpics((current) => {
      const next = new Set(current);
      if (next.has(epicId)) next.delete(epicId);
      else next.add(epicId);
      return next;
    });
  };

  const toggleEpicDescription = (epicId: string) => {
    setExpandedEpicDescriptions((current) => {
      const next = new Set(current);
      if (next.has(epicId)) next.delete(epicId);
      else next.add(epicId);
      return next;
    });
  };

  const handleOpen = (story: Story) => {
    if (dragActivatedRef.current) {
      dragActivatedRef.current = false;
      return;
    }
    setOpen(story);
  };

  const onDragStart = (event: DragStartEvent) => {
    dragActivatedRef.current = true;
    setActiveId(String(event.active.id));
  };

  const onDragCancel = () => {
    setActiveId(null);
    dragActivatedRef.current = false;
  };

  const onDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveId(null);
    if (!over || !repo.data) return;

    const activeType = active.data.current?.type;
    const activeContext = active.data.current?.sourceContext;
    const overContext = over.data.current?.sourceContext ?? over.id;

    if (activeType === "epic" && !backlogReorderDisabled) {
      const oldOrder = epicOrder.map((epic) => epic.id);
      const oldIndex = oldOrder.indexOf(String(active.id));
      const newIndex = oldOrder.indexOf(String(over.id));
      if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
        reorderEpics.mutate({
          orderedIds: arrayMove(oldOrder, oldIndex, newIndex),
          movedId: String(active.id),
          items: epicOrder,
        });
      }
      return;
    }

    if (activeType === "story" && !backlogReorderDisabled && activeContext !== "sprint" && activeContext === overContext) {
      const stories = storiesByEpic.get(String(activeContext)) ?? [];
      const orderedIds = stories.map((story) => story.id);
      const oldIndex = orderedIds.indexOf(String(active.id));
      const newIndex = orderedIds.indexOf(String(over.id));
      if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
        reorderStories.mutate({
          orderedIds: arrayMove(orderedIds, oldIndex, newIndex),
          movedId: String(active.id),
          items: stories,
        });
      }
      return;
    }

    if (activeType === "story" && activeContext === "sprint" && overContext === "sprint") {
      const orderedIds = targetStories.map((story) => story.id);
      const oldIndex = orderedIds.indexOf(String(active.id));
      const newIndex = orderedIds.indexOf(String(over.id));
      if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
        reorderStories.mutate({
          orderedIds: arrayMove(orderedIds, oldIndex, newIndex),
          movedId: String(active.id),
          items: targetStories,
        });
      }
      return;
    }

    const storyId = String(active.id);
    const story = repo.data.stories.find((candidate) => candidate.id === storyId);
    if (!story) return;

    if ((over.id === SPRINT_DROP_ID || overContext === "sprint") && activeContext !== "sprint" && story.sprint !== targetSprint) {
      planStory(storyId);
      return;
    }

    if ((over.id === BACKLOG_DROP_ID || activeContext === "sprint") && story.sprint && (over.id === BACKLOG_DROP_ID || overContext !== "sprint")) {
      unplanStory(storyId);
    }
  };

  if (repo.isLoading) return <div className="view">Loading...</div>;
  if (repo.error) return <div className="view">Failed to load: {String(repo.error)}</div>;

  return (
    <DndContext sensors={sensors} onDragStart={onDragStart} onDragEnd={onDragEnd} onDragCancel={onDragCancel}>
      <div className="view backlog-planning-grid">
        <BacklogDropZone disabled={unplan.isPending}>
          <h2 style={{ marginTop: 0 }}>Backlog</h2>
          <input placeholder="Search stories..." value={search} onChange={(event) => setSearch(event.target.value)} className="field" style={{ marginBottom: 10 }} />
          {backlogReorderDisabled && (
            <p style={{ fontSize: 11, color: "var(--text-faint)", marginTop: 0, marginBottom: 10 }}>
              Priority reordering is disabled while filtering. Planning and story details are still available.
            </p>
          )}
          <SortableContext items={epicOrder.map((epic) => epic.id)} strategy={verticalListSortingStrategy}>
            {epicOrder.map((epic) => (
              <SortableEpicSection
                key={epic.id}
                epic={epic}
                stories={storiesByEpic.get(epic.id) ?? []}
                storiesCollapsed={collapsedEpics.has(epic.id)}
                descriptionExpanded={expandedEpicDescriptions.has(epic.id)}
                backlogReorderDisabled={backlogReorderDisabled}
                targetSprint={targetSprint}
                planPending={plan.isPending}
                onToggleStories={() => toggleEpic(epic.id)}
                onToggleDescription={() => toggleEpicDescription(epic.id)}
                onPlan={planStory}
                onOpen={handleOpen}
              />
            ))}
          </SortableContext>

          {noEpicStories.length > 0 && (
            <div style={{ marginBottom: 12 }} data-testid="epic-section-no-epic">
              <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11, textTransform: "uppercase", color: "var(--text-muted)", marginBottom: 6 }}>
                <span style={{ fontWeight: 700, color: "var(--text)" }}>{NO_EPIC_GROUP_ID}</span>
              </div>
              {noEpicStories.map((story) => (
                <BacklogStoryCard
                  key={story.id}
                  story={story}
                  disabled={!targetSprint || plan.isPending}
                  onPlan={() => planStory(story.id)}
                  onOpen={handleOpen}
                  sourceContext={NO_EPIC_GROUP_ID}
                  sortable={false}
                />
              ))}
            </div>
          )}
        </BacklogDropZone>

        <aside style={{ background: "var(--surface-2)", borderRadius: "var(--radius)", padding: 12 }}>
          <h2 style={{ marginTop: 0 }}>Planning</h2>
          <select value={targetSprint} onChange={(event) => setSprint(event.target.value)} className="field" aria-label="target sprint">
            {sprints.map((candidate) => (
              <option key={candidate.name} value={candidate.name}>{candidate.name} ({candidate.status})</option>
            ))}
          </select>
          {plan.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Plan failed: {String(plan.error)}</div>}
          {unplan.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Remove failed: {String(unplan.error)}</div>}
          {reorderStories.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Story reorder failed: {String(reorderStories.error)}</div>}
          {reorderEpics.error && <div style={{ color: "var(--red)", marginTop: 8 }}>Epic reorder failed: {String(reorderEpics.error)}</div>}
          <p style={{ fontSize: 11, color: "var(--text-faint)", marginTop: 10 }}>
            Click + or drag a story into the sprint box below. Drag sprint stories back to the backlog column or click Remove to unassign them.
          </p>
          <SprintDropZone stories={targetStories} targetSprint={targetSprint} disabled={!targetSprint || plan.isPending || unplan.isPending} onRemove={unplanStory} onOpen={handleOpen} />
        </aside>
      </div>
      <DragOverlay dropAnimation={null}>
        {activeStory && <BacklogStoryOverlay story={activeStory} />}
        {!activeStory && activeEpic && <EpicDragOverlay epic={activeEpic} />}
      </DragOverlay>
      {open && (
        <Suspense fallback={null}>
          <StoryModal story={open} onClose={() => setOpen(null)} statusOptions={BACKLOG_STORY_STATUS_OPTIONS} />
        </Suspense>
      )}
    </DndContext>
  );
}
