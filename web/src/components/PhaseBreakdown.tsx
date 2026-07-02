import { useState } from "react";
import type { Epic, PhaseSummary } from "@shared/types.js";

const PHASE_NAMES: Record<string, string> = {
  F1: "Scaffolding",
  F2: "Core Logic",
  F3: "Administration",
  F4: "Finalization",
  F5: "Go-live Stabilization",
};

function phaseLabel(phase: string): string {
  const name = PHASE_NAMES[phase.toUpperCase()];
  return name ? `${phase} – ${name}` : phase;
}

function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg
      width="10"
      height="10"
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={{
        transition: "transform 0.18s ease",
        transform: open ? "rotate(90deg)" : "rotate(0deg)",
        flexShrink: 0,
      }}
    >
      <polyline points="3,2 9,6 3,10" />
    </svg>
  );
}

interface EpicSummary {
  id: string;
  title: string;
  donePoints: number;
  totalPoints: number;
  doneStories: number;
  totalStories: number;
}

function percent(donePoints: number, totalPoints: number): number {
  return totalPoints === 0 ? 0 : Math.round((donePoints / totalPoints) * 100);
}

function summarizeEpics(epics: Epic[], phase: string): EpicSummary[] {
  return epics
    .filter((epic) => epic.phase === phase)
    .map((epic) => {
      const totals = epic.stories.reduce(
        (acc, story) => {
          const points = story.storyPoints ?? 0;
          if (story.status !== "dropped") {
            acc.totalPoints += points;
            acc.totalStories += 1;
          }
          if (story.status === "done") {
            acc.donePoints += points;
            acc.doneStories += 1;
          }
          return acc;
        },
        { donePoints: 0, totalPoints: 0, doneStories: 0, totalStories: 0 },
      );
      return { id: epic.id, title: epic.title, ...totals };
    })
    .sort((a, b) => percent(b.donePoints, b.totalPoints) - percent(a.donePoints, a.totalPoints));
}

export function PhaseBreakdown({ phases, epics = [] }: { phases: PhaseSummary[]; epics?: Epic[] }) {
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());

  const togglePhase = (phase: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(phase)) next.delete(phase);
      else next.add(phase);
      return next;
    });
  };

  return (
    <div className="card" style={{ cursor: "default" }}>
      <h4 style={{ marginTop: 0 }}>Per-phase breakdown</h4>
      {phases.map((p) => {
        const pct = percent(p.donePoints, p.totalPoints);
        const phaseEpics = summarizeEpics(epics, p.phase);
        const isExpanded = expanded.has(p.phase);
        return (
          <div key={p.phase} className="phase-breakdown-group">
            <button
              type="button"
              className="phase-breakdown-row"
              onClick={() => togglePhase(p.phase)}
              aria-expanded={isExpanded}
            >
              <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span className="phase-breakdown-chevron"><ChevronIcon open={isExpanded} /></span>
                <span className="phase-breakdown-label">{phaseLabel(p.phase)}</span>
              </span>
              <div className="track"><div className="fill done" style={{ width: `${pct}%` }} /></div>
              <span style={{ textAlign: "right", fontSize: 12 }}>{pct}% <span style={{ color: "var(--text-faint)" }}>{p.donePoints}/{p.totalPoints}</span></span>
            </button>
            {isExpanded && (
              <div className="phase-breakdown-epics">
                {phaseEpics.length === 0 ? (
                  <div className="phase-breakdown-empty">No epics in this phase.</div>
                ) : (
                  phaseEpics.map((epic) => {
                    const epicPct = percent(epic.donePoints, epic.totalPoints);
                    return (
                      <div key={epic.id} className="phase-breakdown-epic-row">
                        <span title={epic.title !== epic.id ? `${epic.id}: ${epic.title}` : epic.id}>
                          {epic.id}{epic.title !== epic.id ? `: ${epic.title}` : ""}
                        </span>
                        <div className="track"><div className="fill" style={{ width: `${epicPct}%` }} /></div>
                        <span style={{ textAlign: "right", fontSize: 12 }}>{epicPct}% <span style={{ color: "var(--text-faint)" }}>{epic.donePoints}/{epic.totalPoints}</span></span>
                      </div>
                    );
                  })
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
