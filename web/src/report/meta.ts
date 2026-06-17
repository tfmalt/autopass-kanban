export const PHASE_META: Record<string, { title: string; milestone: string; period: string; priority: string }> = {
  F1: { title: "Phase 1 - Etablering (Establishment)", milestone: "MP1 - Foundation", period: "Q2 2026", priority: "Critical" },
  F2: { title: "Phase 2 - Utvikling: Kjernelogikk (Core Logic)", milestone: "MP2 - Core Logic", period: "Q3 2026", priority: "Critical" },
  F3: { title: "Phase 3 - Utvikling: Administrasjon (Admin)", milestone: "MP3 - Administration", period: "Q4 2026", priority: "High" },
  F4: { title: "Phase 4 - Utvikling: Ferdigstillelse (Completion)", milestone: "MP4 - Complete Functionality", period: "Q1 2027", priority: "High" },
  F5: { title: "Phase 5 - Driftssettelse og Stabilisering", milestone: "MP5 - Production Readiness", period: "Q2 2027", priority: "High" },
};

export const STATUS_LABELS: Record<string, string> = {
  draft: "DRAFT",
  ready: "READY",
  todo: "TODO",
  "in-progress": "IN PROGRESS",
  "ready-for-qa": "READY FOR QA",
  blocked: "BLOCKED",
  done: "DONE",
  dropped: "DROPPED",
};
