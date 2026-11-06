import { PLACE_LIMITS } from "@evinvest/sites-contract";

const TIME = new RegExp(PLACE_LIMITS.time);

/** Minutes since midnight of an `HH:MM`, or null when it is not one. */
export function minutesOf(time: string): number | null {
  return TIME.test(time) ? Number(time.slice(0, 2)) * 60 + Number(time.slice(3)) : null;
}

/** "8:00", "0800", "8h00", "8.00" → "08:00"; anything else stays as typed, for the problem to show. */
export function normaliseTime(raw: string): string {
  const m = /^\s*(\d{1,2})\s*[:h.]?\s*(\d{2})\s*$/i.exec(raw);
  if (!m) return raw.trim();
  const t = `${(m[1] ?? "").padStart(2, "0")}:${m[2] ?? ""}`;
  return minutesOf(t) === null ? raw.trim() : t;
}
