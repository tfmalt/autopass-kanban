import { useDraggable, type DraggableAttributes, type DraggableSyntheticListeners } from "@dnd-kit/core";
import { useSortable } from "@dnd-kit/sortable";
import type { CSSProperties, ReactNode } from "react";
import type { Story } from "@shared/generated/api.js";

export function toTransformStyle(transform: { x: number; y: number } | null, transition?: string, opacity?: number): CSSProperties {
  return {
    transform: transform ? `translate(${transform.x}px, ${transform.y}px)` : undefined,
    transition,
    opacity,
  };
}

function StorySummaryContent({ story }: { story: Story }) {
  return (
    <>
      <div style={{ flex: 1 }}>
        <div className="tid">{story.id}</div>
        <div className="ttl" style={{ margin: 0 }}>{story.title}</div>
      </div>
      <span className="pts">{story.storyPoints ?? "-"}</span>
    </>
  );
}

function StoryCardBody({
  story,
  onOpen,
  dragAttributes,
  dragListeners,
  setNodeRef,
  style,
  beforeContent,
  afterContent,
}: {
  story: Story;
  onOpen: (story: Story) => void;
  dragAttributes: DraggableAttributes;
  dragListeners: DraggableSyntheticListeners;
  setNodeRef: (element: HTMLElement | null) => void;
  style: CSSProperties;
  beforeContent?: ReactNode;
  afterContent?: ReactNode;
}) {
  return (
    <div
      ref={setNodeRef}
      className="card backlog-story-card"
      style={style}
      data-testid={`story-card-${story.id}`}
      {...dragListeners}
      {...dragAttributes}
      onClick={() => onOpen(story)}
    >
      {beforeContent}
      <StorySummaryContent story={story} />
      {afterContent}
    </div>
  );
}

function SortableBacklogStoryCard({ story, disabled, onPlan, onOpen, sourceContext }: { story: Story; disabled: boolean; onPlan: () => void; onOpen: (story: Story) => void; sourceContext: string }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: story.id,
    data: { type: "story", sourceContext },
  });

  return (
    <StoryCardBody
      story={story}
      onOpen={onOpen}
      dragAttributes={attributes}
      dragListeners={listeners}
      setNodeRef={setNodeRef}
      style={toTransformStyle(transform, transition, isDragging ? 0.55 : 1)}
      beforeContent={(
        <button
          aria-label={`add ${story.id}`}
          onClick={(event) => {
            event.stopPropagation();
            onPlan();
          }}
          onPointerDown={(event) => event.stopPropagation()}
          disabled={disabled}
          className="button-add"
        >
          +
        </button>
      )}
    />
  );
}

function DraggableBacklogStoryCard({ story, disabled, onPlan, onOpen, sourceContext }: { story: Story; disabled: boolean; onPlan: () => void; onOpen: (story: Story) => void; sourceContext: string }) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: story.id,
    data: { type: "story", sourceContext },
    disabled,
  });

  return (
    <StoryCardBody
      story={story}
      onOpen={onOpen}
      dragAttributes={attributes}
      dragListeners={listeners}
      setNodeRef={setNodeRef}
      style={toTransformStyle(transform, undefined, isDragging ? 0.55 : 1)}
      beforeContent={(
        <button
          aria-label={`add ${story.id}`}
          onClick={(event) => {
            event.stopPropagation();
            onPlan();
          }}
          onPointerDown={(event) => event.stopPropagation()}
          disabled={disabled}
          className="button-add"
        >
          +
        </button>
      )}
    />
  );
}

export function BacklogStoryCard(props: { story: Story; disabled: boolean; onPlan: () => void; onOpen: (story: Story) => void; sourceContext: string; sortable: boolean }) {
  if (props.sortable) {
    return <SortableBacklogStoryCard {...props} />;
  }
  return <DraggableBacklogStoryCard {...props} />;
}

export function SprintStoryCard({ story, disabled, onRemove, onOpen }: { story: Story; disabled: boolean; onRemove: () => void; onOpen: (story: Story) => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: story.id,
    data: { type: "story", sourceContext: "sprint" },
    disabled,
  });

  return (
    <StoryCardBody
      story={story}
      onOpen={onOpen}
      dragAttributes={attributes}
      dragListeners={listeners}
      setNodeRef={setNodeRef}
      style={toTransformStyle(transform, transition, isDragging ? 0.55 : 1)}
      afterContent={(
        <button
          aria-label={`remove ${story.id}`}
          onClick={(event) => {
            event.stopPropagation();
            onRemove();
          }}
          onPointerDown={(event) => event.stopPropagation()}
          disabled={disabled}
          className="button-add"
        >
          Remove
        </button>
      )}
    />
  );
}

export function BacklogStoryOverlay({ story }: { story: Story }) {
  return (
    <div data-testid="backlog-drag-overlay">
      <div className="card card--overlay" style={{ marginBottom: 0 }}>
        <StorySummaryContent story={story} />
      </div>
    </div>
  );
}
