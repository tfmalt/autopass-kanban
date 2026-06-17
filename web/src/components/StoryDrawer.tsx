import type { Story } from "@shared/types.js";

export function StoryDrawer({ story, onClose }: { story: Story; onClose: () => void }) {
  return (
    <div className="overlay" onClick={onClose}>
      <div className="drawer" onClick={(e) => e.stopPropagation()}>
        <div className="tid">{story.id} · {story.storyPoints ?? "-"} pts</div>
        <h2 style={{ marginTop: 4 }}>{story.title}</h2>
        <p style={{ color: "var(--text-muted)", fontSize: 13 }}>
          {story.status} · {(story.assignee && story.assignee.trim()) || "unassigned"} · {story.epic ?? "no epic"}
        </p>
        <h3 style={{ fontSize: 13 }}>Tasks ({story.taskSummary.done}/{story.taskSummary.total})</h3>
        {story.tasks.map((task) => (
          <div key={task.id} style={{ border: "1px solid var(--border)", borderRadius: 8, padding: 8, marginBottom: 6 }}>
            <div style={{ fontSize: 11, color: "var(--text-faint)" }}>{task.id} · {task.status}</div>
            <div style={{ fontSize: 12.5 }}>{task.title}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
