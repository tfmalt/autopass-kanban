import { STORY_STATUSES } from "./generated/api.js";
import type { StoryStatus } from "./generated/api.js";

export {
  SPRINT_STATUSES,
  STORY_LIFECYCLE_STATUSES,
  STORY_STATUSES,
  TASK_STATUSES,
} from "./generated/api.js";
export type {
  SprintStatus,
  StoryLifecycleStatus,
  StoryStatus,
  TaskStatus,
} from "./generated/api.js";

const ASSIGNEE_PLACEHOLDER = /^Name <email@example\.com>$/i;

export function normalizeStatus(value: string): string {
  return value.toLowerCase().trim();
}

export function isBoardStatus(value: string): value is StoryStatus {
  return (STORY_STATUSES as string[]).includes(value);
}

export function parseAssignees(value: string | string[] | null | undefined): string[] {
  const values = Array.isArray(value) ? value : typeof value === "string" ? value.split(",") : [];
  return values
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)
    .filter((entry) => entry !== "~")
    .filter((entry) => entry.toUpperCase() !== "TBD")
    .filter((entry) => !ASSIGNEE_PLACEHOLDER.test(entry));
}

export function abbreviateAssignee(value: string): string {
  const name = value.split("<", 1)[0]?.trim() ?? "";
  const firstWord = name.split(/\s+/, 1)[0] ?? "";
  const short = firstWord || name || value.trim();
  return short.slice(0, 6);
}

// Keep these cases aligned with crates/core/src/util.rs tests.
export function slugifyHeadline(value: string): string {
  let slug = "";
  let lastWasDash = false;
  for (const ch of value.trim()) {
    const normalized = ch.toLowerCase();
    if (/^[a-z0-9]$/.test(normalized)) {
      slug += normalized;
      lastWasDash = false;
    } else if (!lastWasDash && slug.length > 0) {
      slug += "-";
      lastWasDash = true;
    }
  }
  return slug.replace(/^-+|-+$/g, "");
}
