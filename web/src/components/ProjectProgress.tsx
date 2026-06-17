import type { ProjectProgress as Progress } from "@shared/types.js";

export function ProjectProgress({ progress }: { progress: Progress }) {
  const pct = progress.totalPoints === 0 ? 0 : Math.round((progress.donePoints / progress.totalPoints) * 100);
  return (
    <div style={{ minWidth: 320 }}>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 10.5, color: "var(--text-muted)", marginBottom: 4 }}>
        <span>Project completion</span>
        <span>{progress.donePoints} / {progress.totalPoints} pts · {pct}%</span>
      </div>
      <div className="track">
        <div className="fill done" style={{ width: `${pct}%` }} />
      </div>
      <div style={{ fontSize: 9.5, color: "var(--text-faint)", marginTop: 3 }}>
        {progress.doneStories} of {progress.totalStories} stories done
      </div>
    </div>
  );
}
