import { useSortable } from "@dnd-kit/sortable";
import { useMemo, type CSSProperties } from "react";
import { type Story, type StoryStatus, type TeamMember } from "@shared/types.js";
import { useTeam } from "../api/hooks.js";

const AVATAR_PALETTE = [
  { bg: "#FDE68A", fg: "#92400E" },
  { bg: "#A7F3D0", fg: "#065F46" },
  { bg: "#BFDBFE", fg: "#1E40AF" },
  { bg: "#FBCFE8", fg: "#9D174D" },
  { bg: "#DDD6FE", fg: "#5B21B6" },
  { bg: "#FED7AA", fg: "#9A3412" },
  { bg: "#99F6E4", fg: "#115E59" },
  { bg: "#FECACA", fg: "#991B1B" },
  { bg: "#D9F99D", fg: "#3F6212" },
  { bg: "#F5D0FE", fg: "#86198F" },
];

function pickColor(key: string): { bg: string; fg: string } {
  let hash = 0;
  for (let i = 0; i < key.length; i++) {
    hash = (hash << 5) - hash + key.charCodeAt(i);
  }
  return AVATAR_PALETTE[Math.abs(hash) % AVATAR_PALETTE.length]!;
}

function avatarColors(email: string): { bgStyle: CSSProperties; fgStyle: CSSProperties } {
  const c = pickColor(email || "default");
  return {
    bgStyle: { backgroundColor: c.bg },
    fgStyle: { color: c.fg },
  };
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .map((w) => w[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);
}

function useAssigneeMap(): Map<string, TeamMember> {
  const team = useTeam();
  return useMemo(() => {
    const m = new Map<string, TeamMember>();
    for (const member of team.data ?? []) {
      m.set(member.email, member);
    }
    return m;
  }, [team.data]);
}

function CardContent({ story }: { story: Story }) {
  const assigneeMap = useAssigneeMap();
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
        <div className="assignee-avatars">
        {story.assignees.map((assignee) => {
          const email = assignee.match(/<([^>]+)>/)?.[1] ?? "";
          const member = assigneeMap.get(email);
          const url = member?.avatarUrl;
          const letter = member ? initials(member.name) : assignee.slice(0, 2).toUpperCase();
          const { bgStyle, fgStyle } = avatarColors(email);
          return (
            <span key={assignee} className="assignee-avatar" title={assignee} style={bgStyle}>
              {url ? (
                <img src={url} alt={member!.name} className="assignee-avatar-img" />
              ) : (
                <span className="assignee-avatar-initials" style={fgStyle}>{letter}</span>
              )}
            </span>
          );
        })}
      </div>
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
