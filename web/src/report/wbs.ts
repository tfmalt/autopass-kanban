import type { RepositorySnapshot, Story } from "@shared/types.js";
import { normalizeStatus } from "@shared/types.js";
import { parseDate } from "@shared/dates.js";
import { PHASE_META, STATUS_LABELS } from "./meta.js";
import { groupDates, roundMetric, sumPoints } from "./estimates.js";
import type { Estimate } from "./estimates.js";

export interface WbsRow {
  kind: "phase" | "epic" | "story";
  wbs: string;
  id: string;
  title: string;
  milestone: string;
  period: string;
  priority: string;
  status: string;
  points: number | null;
  estHours: number | null;
  startDate: string | null;
  endDate: string | null;
  notes: string;
}

export interface PhaseGroup {
  id: string;
  epics: Array<{ id: string; title: string; stories: Story[] }>;
}

export function buildHierarchy(repo: RepositorySnapshot): PhaseGroup[] {
  const epicTitles = new Map(repo.epics.map((epic) => [epic.id, epic.title]));
  const phaseMap = new Map<string, Map<string, { title: string; stories: Story[] }>>();
  for (const story of repo.stories) {
    const phase = story.phase ?? story.id.split("-")[1] ?? "unknown";
    const epicId = story.epic ?? `(no epic in ${phase})`;
    const epicMap = phaseMap.get(phase) ?? new Map<string, { title: string; stories: Story[] }>();
    const epic = epicMap.get(epicId) ?? { title: epicTitles.get(epicId) ?? epicId, stories: [] };
    epic.stories.push(story);
    epicMap.set(epicId, epic);
    phaseMap.set(phase, epicMap);
  }

  return [...phaseMap.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([id, epics]) => ({
    id,
    epics: [...epics.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([epicId, epic]) => ({
      id: epicId,
      title: epic.title,
      stories: [...epic.stories].sort((a, b) => a.id.localeCompare(b.id)),
    })),
  }));
}

export function buildWbsRows(repo: RepositorySnapshot, estimates: Map<string, Estimate>, hoursPerPoint: number): WbsRow[] {
  const rows: WbsRow[] = [];
  buildHierarchy(repo).forEach((phase, phaseIndex) => {
    const phaseWbs = String(phaseIndex + 1);
    const meta = PHASE_META[phase.id] ?? { title: phase.id, milestone: "", period: "", priority: "" };
    const phaseStories = phase.epics.flatMap((epic) => epic.stories);
    const phaseDates = groupDates(phaseStories, estimates);
    rows.push({
      kind: "phase",
      wbs: phaseWbs,
      id: phase.id,
      title: meta.title,
      milestone: meta.milestone,
      period: meta.period,
      priority: meta.priority,
      status: "",
      points: sumPoints(phaseStories),
      estHours: null,
      startDate: phaseDates.start,
      endDate: phaseDates.end,
      notes: "",
    });

    phase.epics.forEach((epic, epicIndex) => {
      const epicWbs = `${phaseWbs}.${epicIndex + 1}`;
      const epicDates = groupDates(epic.stories, estimates);
      rows.push({
        kind: "epic",
        wbs: epicWbs,
        id: epic.id,
        title: epic.title,
        milestone: meta.milestone,
        period: meta.period,
        priority: meta.priority,
        status: "",
        points: sumPoints(epic.stories),
        estHours: null,
        startDate: epicDates.start,
        endDate: epicDates.end,
        notes: "",
      });

      epic.stories.forEach((story, storyIndex) => {
        const status = normalizeStatus(story.status);
        const estimate = estimates.get(story.id);
        const activeOrDone = ["done", "in-progress", "ready-for-qa"].includes(status);
        rows.push({
          kind: "story",
          wbs: `${epicWbs}.${storyIndex + 1}`,
          id: story.id,
          title: story.title,
          milestone: meta.milestone,
          period: meta.period,
          priority: meta.priority,
          status: STATUS_LABELS[status] ?? status.toUpperCase(),
          points: story.storyPoints,
          estHours: activeOrDone && story.storyPoints && hoursPerPoint > 0
            ? roundMetric(story.storyPoints * hoursPerPoint)
            : estimate?.estHours ?? null,
          startDate: status === "done" || activeOrDone ? parseDate(story.workStarted) ?? estimate?.estStart ?? null : estimate?.estStart ?? null,
          endDate: status === "done" ? parseDate(story.workDone) : estimate?.estEnd ?? null,
          notes: [story.sprint ? `Sprint ${story.sprint}` : null, story.assignee ? `Assignee ${story.assignee}` : null].filter(Boolean).join("; "),
        });
      });
    });
  });
  return rows;
}
