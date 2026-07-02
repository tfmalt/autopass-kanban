import { useDroppable } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import type { ReactNode } from "react";
import type { Story } from "@shared/generated/api.js";
import { BACKLOG_DROP_ID, SPRINT_DROP_ID } from "./constants.js";
import { SprintStoryCard } from "./BacklogStoryCard.js";

export function BacklogDropZone({ children, disabled }: { children: ReactNode; disabled: boolean }) {
  const { setNodeRef, isOver } = useDroppable({ id: BACKLOG_DROP_ID, disabled });
  return (
    <section ref={setNodeRef} className={`backlog-column${isOver ? " is-over" : ""}`}>
      {children}
    </section>
  );
}

export function SprintDropZone({ stories, targetSprint, disabled, onRemove, onOpen }: { stories: Story[]; targetSprint: string; disabled: boolean; onRemove: (storyId: string) => void; onOpen: (story: Story) => void }) {
  const { setNodeRef, isOver } = useDroppable({ id: SPRINT_DROP_ID, disabled });
  const points = stories.reduce((sum, story) => sum + (story.storyPoints ?? 0), 0);

  return (
    <div ref={setNodeRef} aria-label="current sprint drop target" className={`sprint-dropzone${isOver ? " is-over" : ""}`}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "baseline" }}>
        <h3 style={{ margin: 0 }}>User Stories for Current Sprint</h3>
        <span className="pts">{stories.length} · {points} pts</span>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>{targetSprint || "No sprint selected"}</div>
      <div style={{ fontSize: 11, color: "var(--text-faint)", marginTop: 10 }}>
        Drag backlog stories here to plan them into the selected sprint.
      </div>
      <div style={{ marginTop: 12 }}>
        {stories.length === 0 ? (
          <div style={{ fontSize: 12, color: "var(--text-muted)" }}>No user stories planned for this sprint yet.</div>
        ) : (
          <SortableContext items={stories.map((story) => story.id)} strategy={verticalListSortingStrategy}>
            {stories.map((story) => (
              <SprintStoryCard key={story.id} story={story} disabled={disabled} onRemove={() => onRemove(story.id)} onOpen={onOpen} />
            ))}
          </SortableContext>
        )}
      </div>
    </div>
  );
}
