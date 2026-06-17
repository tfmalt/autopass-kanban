/**
 * UTC date-string helpers shared between the server and the client.
 * All helpers that operate on YYYY-MM-DD strings use UTC midnight to avoid
 * local-timezone drift; `daysBetween` and `dateToTime` also accept full ISO
 * timestamps so they work correctly with workStarted/workDone values that
 * include a time-and-offset component.
 */

/** Milliseconds per day. */
export const DAY_MS = 86_400_000;

/** Returns the current date as a YYYY-MM-DD string (UTC). */
export function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

/**
 * Number of whole calendar days from `a` to `b`.
 * Accepts both YYYY-MM-DD strings and full ISO timestamps.
 * Returns 0 if `b` is before `a`.
 */
export function daysBetween(a: string, b: string): number {
  const ms = new Date(b).getTime() - new Date(a).getTime();
  return Math.max(0, Math.round(ms / DAY_MS));
}

/**
 * Unix timestamp (ms) for a YYYY-MM-DD string at UTC midnight.
 * Also accepts full ISO timestamps.
 */
export function dateToTime(date: string): number {
  return new Date(`${date}T00:00:00Z`).getTime();
}

/** Advances a YYYY-MM-DD string by `days` calendar days (UTC). */
export function addDays(date: string, days: number): string {
  const d = new Date(`${date}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

/** Returns true when the given YYYY-MM-DD string falls on Mon–Fri (UTC). */
export function isWeekday(date: string): boolean {
  const day = new Date(`${date}T00:00:00Z`).getUTCDay();
  return day >= 1 && day <= 5;
}

/** Advances a YYYY-MM-DD string by `days` working days (Mon–Fri, UTC). */
export function addWorkingDays(date: string, days: number): string {
  let current = date;
  let remaining = days;
  while (remaining > 0) {
    current = addDays(current, 1);
    if (isWeekday(current)) remaining -= 1;
  }
  return current;
}

/** Counts working days (Mon–Fri) in the inclusive range [start, end]. */
export function workDaysInclusive(start: string, end: string): number {
  let days = 0;
  let cursor = start;
  while (cursor <= end) {
    if (isWeekday(cursor)) days += 1;
    cursor = addDays(cursor, 1);
  }
  return days;
}

/**
 * Returns the YYYY-MM-DD prefix of a date string, or null when the input is
 * absent or does not start with a valid YYYY-MM-DD segment.
 */
export function parseDate(value: string | null | undefined): string | null {
  if (!value) return null;
  const date = value.slice(0, 10);
  return /^\d{4}-\d{2}-\d{2}$/.test(date) ? date : null;
}

/** Formats a Unix timestamp (ms) as a YYYY-MM-DD string (UTC). */
export function formatDate(time: number): string {
  return new Date(time).toISOString().slice(0, 10);
}
