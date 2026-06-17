import { useSortable } from "@dnd-kit/sortable";
import { abbreviateAssignee, type Story, type StoryStatus } from "@shared/types.js";

function CardContent({ story }: { story: Story }) {
  return (
    <>
      <div className="tid">{story.id}</div>
      <div className="ttl">{story.title}</div>
      <div className="card-chip-row">
        {story.storyPoints != null && <span className="pts">{story.storyPoints} pts</span>}
        {story.taskSummary.total > 0 && (
          <span className="story-chip story-chip--done">
            {story.taskSummary.done}/{story.taskSummary.total} done
          </span>
        )}
        {story.assignees.map((assignee) => (
          <span key={assignee} className="story-chip story-chip--assignee" title={assignee}>
            {abbreviateAssignee(assignee)}
          </span>
        ))}
      </div>
    </>
  );
}

export function StoryCard({ story, status, onOpen }: { story: Story; status: StoryStatus; onOpen?: (story: Story) => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: story.id,
    data: { type: "story", status },
  });
  const style = {
    transform: transform ? `translate(${transform.x}px, ${transform.y}px)` : undefined,
    transition,
    opacity: isDragging ? 0.55 : 1,
  };
  return (
    <div
      ref={setNodeRef}
      className={`card${isDragging ? " card--ghost" : ""}`}
      style={style}
      {...listeners}
      {...attributes}
      onClick={() => onOpen?.(story)}
    >
      <CardContent story={story} />
    </div>
  );
}

/** Non-interactive card rendered inside DragOverlay — follows the cursor with a raised look. */
export function StoryCardOverlay({ story }: { story: Story }) {
  return (
    <div className="card card--overlay">
      <CardContent story={story} />
    </div>
  );
}
