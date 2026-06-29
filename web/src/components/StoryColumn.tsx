import { useDroppable } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import type { Story, StoryStatus } from "@shared/types.js";
import { StoryCard } from "./StoryCard.js";

const LABEL: Record<StoryStatus, string> = {
  planned: "Planned",
  todo: "To Do",
  "in-progress": "In Progress",
  "ready-for-qa": "Ready for QA",
  done: "Done",
  blocked: "Blocked",
};

export function StoryColumn({
  status,
  stories,
  onOpen,
  activeDragId,
}: {
  status: StoryStatus;
  stories: Story[];
  onOpen?: (s: Story) => void;
  activeDragId?: string | null;
}) {
  const { setNodeRef, isOver } = useDroppable({ id: status, data: { type: "column", status } });
  const points = stories.reduce((sum, s) => sum + (s.storyPoints ?? 0), 0);
  // Show a drop placeholder when hovering and the dragged card is not already in this column.
  const showPlaceholder = isOver && activeDragId != null && !stories.some((s) => s.id === activeDragId);
  return (
    <div ref={setNodeRef} className={`column${status === "blocked" ? " blocked" : ""}${isOver ? " is-over" : ""}`}>
      <h4>
        <span>{LABEL[status]}</span>
        <span>{stories.length} · {points} pts</span>
      </h4>
      <SortableContext items={stories.map((story) => story.id)} strategy={verticalListSortingStrategy}>
        {stories.map((story) => (
          <StoryCard key={story.id} story={story} status={status} onOpen={onOpen} />
        ))}
      </SortableContext>
      {showPlaceholder && <div className="card-placeholder" />}
    </div>
  );
}
