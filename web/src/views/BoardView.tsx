import { useRef, useState } from "react";
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
import { arrayMove, sortableKeyboardCoordinates } from "@dnd-kit/sortable";
import { isBoardStatus, STORY_STATUSES, type Story } from "@shared/types.js";
import { useMoveStory, useReorderStories, useRepository } from "../api/hooks.js";
import { StoryCardOverlay } from "../components/StoryCard.js";
import { StoryColumn } from "../components/StoryColumn.js";
import { StoryModal } from "../components/StoryModal.js";

const BOARD_STATUSES = STORY_STATUSES.filter((status) => status !== "planned");

export function BoardView() {
  const repo = useRepository();
  const move = useMoveStory();
  const reorderStories = useReorderStories();
  const [open, setOpen] = useState<Story | null>(null);
  const [selectedSprint, setSelectedSprint] = useState("");
  const [activeStory, setActiveStory] = useState<Story | null>(null);
  // Guard: suppress click events that fire right after a drag completes.
  const dragActivatedRef = useRef(false);

  // Require the pointer to move at least 8px before starting a drag.
  // This lets short taps fire the click handler (which opens the modal)
  // without accidentally triggering a drag.
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  if (repo.isLoading) return <div className="view">Loading...</div>;
  if (repo.error) return <div className="view">Failed to load: {String(repo.error)}</div>;

  const sprints = repo.data!.sprints;
  const defaultSprint = sprints.find((s) => s.status === "active") ?? sprints.at(-1);
  const visibleSprint = sprints.find((s) => s.name === selectedSprint) ?? defaultSprint;
  if (!visibleSprint) return <div className="view">No sprint found.</div>;

  const allStories = STORY_STATUSES.flatMap((s) => visibleSprint.storiesByStatus[s]);

  const onDragStart = (event: DragStartEvent) => {
    dragActivatedRef.current = true;
    const id = String(event.active.id);
    setActiveStory(allStories.find((s) => s.id === id) ?? null);
  };

  const onDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveStory(null);
    if (!over) return;

    const id = String(active.id);
    const activeStatus = active.data.current?.status;
    const overStatus = over.data.current?.status ?? over.id;
    if (typeof activeStatus !== "string" || !isBoardStatus(activeStatus) || typeof overStatus !== "string" || !isBoardStatus(overStatus)) {
      return;
    }

    if (activeStatus === overStatus) {
      const stories = visibleSprint.storiesByStatus[activeStatus];
      const orderedIds = stories.map((story) => story.id);
      const oldIndex = orderedIds.indexOf(id);
      const newIndex = orderedIds.indexOf(String(over.id));
      if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
        reorderStories.mutate({
          orderedIds: arrayMove(orderedIds, oldIndex, newIndex),
          movedId: id,
          items: stories,
        });
      }
      return;
    }

    move.mutate({ id, status: overStatus });

    const story = allStories.find((candidate) => candidate.id === id);
    const targetStories = visibleSprint.storiesByStatus[overStatus];
    const targetIndex = targetStories.findIndex((candidate) => candidate.id === String(over.id));
    if (story && targetIndex !== -1) {
      const orderedIds = targetStories.map((candidate) => candidate.id);
      orderedIds.splice(targetIndex, 0, id);
      reorderStories.mutate({
        orderedIds,
        movedId: id,
        items: [...targetStories, story],
      });
    }
  };

  const onDragCancel = () => {
    setActiveStory(null);
    dragActivatedRef.current = false;
  };

  const handleOpen = (story: Story) => {
    // If a drag just completed, the pointer-up/click event still fires — swallow it.
    if (dragActivatedRef.current) {
      dragActivatedRef.current = false;
      return;
    }
    setOpen(story);
  };

  return (
    <div className="view">
      <div style={{ display: "flex", alignItems: "flex-end", justifyContent: "space-between", gap: 16, flexWrap: "wrap", marginBottom: 12 }}>
        <h2 style={{ margin: 0, paddingBottom: 10 }}>
          {visibleSprint.name} <span style={{ fontSize: 12, color: "var(--text-muted)" }}>{visibleSprint.startDate} → {visibleSprint.endDate}</span>
        </h2>
        <label style={{ display: "block", width: 360, maxWidth: "100%" }}>
          <span style={{ display: "block", fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>Sprint</span>
          <select aria-label="sprint" value={visibleSprint.name} onChange={(e) => setSelectedSprint(e.target.value)} className="field">
            {sprints.map((sprint) => (
              <option key={sprint.name} value={sprint.name}>{sprint.name} ({sprint.status ?? "unknown"})</option>
            ))}
          </select>
        </label>
      </div>
      {move.error && <div style={{ color: "var(--red)", marginBottom: 8 }}>Move failed: {String(move.error)}</div>}
      {reorderStories.error && <div style={{ color: "var(--red)", marginBottom: 8 }}>Story reorder failed: {String(reorderStories.error)}</div>}
      <DndContext sensors={sensors} onDragStart={onDragStart} onDragEnd={onDragEnd} onDragCancel={onDragCancel}>
        <div className="columns">
          {BOARD_STATUSES.map((status) => (
            <StoryColumn
              key={status}
              status={status}
              stories={visibleSprint.storiesByStatus[status]}
              onOpen={handleOpen}
              activeDragId={activeStory?.id ?? null}
            />
          ))}
        </div>
        <DragOverlay dropAnimation={null}>
          {activeStory && <StoryCardOverlay story={activeStory} />}
        </DragOverlay>
      </DndContext>
      {open && <StoryModal story={open} onClose={() => setOpen(null)} />}
    </div>
  );
}
