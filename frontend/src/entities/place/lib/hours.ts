import { DAYS, type DayOfWeek, type OpeningHours } from "@evinvest/sites-contract";

import type { T } from "@/shared/i18n";
import { minutesOf } from "@/shared/lib/clock";

export interface LocalTime {
  day: DayOfWeek;
  minutes: number;
}

/** The weekday and minute of `at` on the place's wall clock. */
export function localTimeOf(at: Date, timeZone: string): LocalTime {
  const parts = new Intl.DateTimeFormat("en-US", { timeZone, weekday: "long", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).formatToParts(at);
  const part = (type: string) => parts.find((p) => p.type === type)?.value ?? "";
  const day = DAYS.find((d) => d === part("weekday")) ?? "Monday";
  return { day, minutes: Number(part("hour")) * 60 + Number(part("minute")) };
}

/** Whether the hours say "open" at a local time. */
export function isOpenAt(hours: readonly OpeningHours[], at: LocalTime): boolean {
  return hours.some((row) => {
    const opens = minutesOf(row.opens);
    const closes = minutesOf(row.closes);
    return row.days.includes(at.day) && opens !== null && closes !== null && at.minutes >= opens && at.minutes < closes;
  });
}

/** Days in week order, runs of three or more joined: "Mon–Fri, Sun". */
export function formatDays(days: readonly DayOfWeek[], t: T): string {
  const idx = [...new Set(days.map((d) => DAYS.indexOf(d)))].sort((a, b) => a - b);
  const runs: number[][] = [];
  for (const i of idx) {
    const run = runs.at(-1);
    if (run && run.at(-1) === i - 1) run.push(i);
    else runs.push([i]);
  }
  const name = (i: number) => t(`day.short.${DAYS[i] ?? "Monday"}`);
  return runs
    .flatMap((run) => (run.length >= 3 ? [`${name(run[0] ?? 0)}–${name(run.at(-1) ?? 0)}`] : run.map(name)))
    .join(", ");
}

/** "Mon–Fri 08:00–19:00; Sat 09:00–12:00". */
export function formatHours(hours: readonly OpeningHours[], t: T): string {
  return hours.map((row) => `${formatDays(row.days, t)} ${row.opens}–${row.closes}`).join("; ");
}
