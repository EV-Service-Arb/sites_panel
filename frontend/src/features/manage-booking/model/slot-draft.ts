import type { DayPart } from "@evinvest/sites-contract";

import type { LeadBooking, SlotBody } from "@/entities/lead";
import { minutesOf, normaliseTime } from "@/shared/lib/clock";
import { dayOfDate, localInstant, rfc3339 } from "@/shared/lib/instant";

/** The slot as typed, in the browser's zone: a day, a start, an optional end. */
export interface SlotDraft {
  day: string;
  start: string;
  end: string;
}

export type SlotProblem = "day" | "start" | "end" | "order";

/** Where a part of the day the visitor asked for begins, as a first guess the operator corrects. */
const PART_STARTS: Record<DayPart, string> = { morning: "09:00", afternoon: "14:00", evening: "18:00" };

const hhmm = (d: Date) => `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;

/** The slot booked, else the visitor's wish, else nothing yet: the operator moves from what is known. */
export function slotDraftOf(b: LeadBooking): SlotDraft {
  const start = b.start_at === null ? null : new Date(b.start_at);
  if (start && !Number.isNaN(start.getTime())) {
    const end = b.end_at === null ? null : new Date(b.end_at);
    return { day: dayOfDate(start), start: hhmm(start), end: end && !Number.isNaN(end.getTime()) ? hhmm(end) : "" };
  }
  return { day: b.preferred_date ?? "", start: b.preferred_part ? PART_STARTS[b.preferred_part] : "", end: "" };
}

/** Every problem at once, so the form marks each field; empty when the slot can be sent. */
export function slotProblems(d: SlotDraft): SlotProblem[] {
  const problems: SlotProblem[] = [];
  const start = minutesOf(normaliseTime(d.start));
  const end = d.end.trim() === "" ? null : minutesOf(normaliseTime(d.end));
  if (localInstant(d.day, "00:00") === null) problems.push("day");
  if (start === null) problems.push("start");
  if (d.end.trim() !== "" && end === null) problems.push("end");
  // One day only: a visit that runs past midnight is not a booking this form makes.
  if (start !== null && end !== null && end <= start) problems.push("order");
  return problems;
}

/**
 * The body `POST …/booking` takes, the instants RFC 3339 with the browser's
 * offset at each (a slot after the autumn change carries the winter one). Null
 * while the draft has a problem.
 */
export function slotBody(d: SlotDraft, offsetOf?: (at: Date) => number): SlotBody | null {
  if (slotProblems(d).length > 0) return null;
  const start = localInstant(d.day, normaliseTime(d.start));
  const end = d.end.trim() === "" ? null : localInstant(d.day, normaliseTime(d.end));
  if (start === null) return null;
  const at = (x: Date) => (offsetOf ? rfc3339(x, offsetOf(x)) : rfc3339(x));
  return end === null ? { action: "set", start_at: at(start) } : { action: "set", start_at: at(start), end_at: at(end) };
}

/** The browser's zone as people know it: "Europe/Paris". */
export function browserZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}
