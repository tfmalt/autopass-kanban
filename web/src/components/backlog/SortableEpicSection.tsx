import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import type { Epic, Story } from "@shared/generated/api.js";
import { BacklogStoryCard, toTransformStyle } from "./BacklogStoryCard.js";
import { EpicContext } from "./EpicContext.js";

function EpicChevron({ expanded }: { expanded: boolean }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 12 12"
      width="12"
      height="12"
      style={{
        flex: "0 0 auto",
        transform: expanded ? "rotate(90deg)" : "rotate(0deg)",
        transition: "transform 120ms ease",
      }}
    >
      <path d="M4 2.5 8 6 4 9.5" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function EpicDragOverlay({ epic }: { epic: Epic }) {
  return (
    <div className="card card--overlay" data-testid="backlog-drag-overlay" style={{ marginBottom: 0 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 11, textTransform: "uppercase", color: "var(--text-muted)" }}>
        <span className="drag-handle" aria-hidden="true">::</span>
        <span style={{ fontWeight: 700, color: "var(--text)" }}>{epic.id}</span>
        <span style={{ color: "var(--text-muted)" }}>{epic.title}</span>
      </div>
    </div>
  );
}

export function SortableEpicSection({
  epic,
  stories,
  storiesCollapsed,
  descriptionExpanded,
  backlogReorderDisabled,
  targetSprint,
  planPending,
  onToggleStories,
  onToggleDescription,
  onPlan,
  onOpen,
}: {
  epic: Epic;
  stories: Story[];
  storiesCollapsed: boolean;
  descriptionExpanded: boolean;
  backlogReorderDisabled: boolean;
  targetSprint: string;
  planPending: boolean;
  onToggleStories: () => void;
  onToggleDescription: () => void;
  onPlan: (storyId: string) => void;
  onOpen: (story: Story) => void;
}) {
  const { attributes, listeners, setActivatorNodeRef, setNodeRef, transform, transition, isDragging } = useSortable({
    id: epic.id,
    data: { type: "epic" },
    disabled: backlogReorderDisabled,
  });

  return (
    <div
      ref={setNodeRef}
      style={{
        ...toTransformStyle(transform, transition, isDragging ? 0.75 : 1),
        marginBottom: 12,
      }}
      data-testid={`epic-section-${epic.id}`}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
        <span
          ref={setActivatorNodeRef}
          className="drag-handle"
          aria-label={`reorder epic ${epic.id}`}
          onClick={(event) => event.stopPropagation()}
          {...listeners}
          {...attributes}
        >
          ::
        </span>
        <button
          type="button"
          onClick={onToggleStories}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            fontSize: 11,
            textTransform: "uppercase",
            color: "var(--text-muted)",
            border: 0,
            background: "none",
            padding: 0,
            cursor: "pointer",
          }}
          aria-expanded={!storiesCollapsed}
          aria-label={`${storiesCollapsed ? "expand" : "collapse"} user stories for ${epic.id}`}
        >
          <EpicChevron expanded={!storiesCollapsed} />
          <span style={{ fontWeight: 700, color: "var(--text)" }}>{epic.id}</span>
          <span style={{ color: "var(--text-muted)" }}>{epic.title}</span>
          {storiesCollapsed && <span className="epic-story-count">({stories.length} stories)</span>}
        </button>
        <button
          type="button"
          onClick={onToggleDescription}
          className="epic-description-toggle"
          aria-expanded={descriptionExpanded}
          aria-label={descriptionExpanded ? `hide epic description ${epic.id}` : `show epic description ${epic.id}`}
        >
          {descriptionExpanded ? "Hide description" : "Description"}
        </button>
      </div>
      {descriptionExpanded && <EpicContext epicId={epic.id} />}
      {!storiesCollapsed && (
        <SortableContext items={stories.map((story) => story.id)} strategy={verticalListSortingStrategy}>
          {stories.map((story) => (
            <BacklogStoryCard
              key={story.id}
              story={story}
              disabled={!targetSprint || planPending}
              onPlan={() => onPlan(story.id)}
              onOpen={onOpen}
              sourceContext={epic.id}
              sortable={!backlogReorderDisabled}
            />
          ))}
        </SortableContext>
      )}
    </div>
  );
}
