import { DAYS, type DayOfWeek, type OpeningHours, type PlaceProblem, checkPlaceLive } from "@evinvest/sites-contract";

import { minutesOf } from "@/shared/lib/clock";

/** A row as typed: the times are raw text until they parse. `key` keeps React's rows stable. */
export interface HoursDraftRow {
  key: number;
  days: DayOfWeek[];
  opens: string;
  closes: string;
}

export type RowProblem = { code: PlaceProblem["code"] } | { code: "overlap"; days: DayOfWeek[] };

let nextKey = 0;
const key = () => ++nextKey;

export function hoursDraftOf(hours: readonly OpeningHours[] | undefined): HoursDraftRow[] {
  return (hours ?? []).map((row) => ({ key: key(), days: [...row.days], opens: row.opens, closes: row.closes }));
}

/** A new row takes the days no row has yet, so "Mon–Fri" then "Sat" is two clicks fewer. */
export function addHoursRow(rows: readonly HoursDraftRow[]): HoursDraftRow[] {
  const taken = new Set(rows.flatMap((r) => r.days));
  return [...rows, { key: key(), days: DAYS.filter((d) => !taken.has(d)), opens: "09:00", closes: "18:00" }];
}

export function updateHoursRow(rows: readonly HoursDraftRow[], rowKey: number, patch: Partial<Omit<HoursDraftRow, "key">>): HoursDraftRow[] {
  return rows.map((r) => (r.key === rowKey ? { ...r, ...patch } : r));
}

export function removeHoursRow(rows: readonly HoursDraftRow[], rowKey: number): HoursDraftRow[] {
  return rows.filter((r) => r.key !== rowKey);
}

export { normaliseTime } from "@/shared/lib/clock";

function span(row: HoursDraftRow): { from: number; to: number } | null {
  const from = minutesOf(row.opens);
  const to = minutesOf(row.closes);
  return from !== null && to !== null && from < to ? { from, to } : null;
}

/** What stops a row from saving: the contract's reasons, then the editor's own — two rows on one day are fine (a lunch break) unless their times overlap. */
export function rowProblems(rows: readonly HoursDraftRow[], row: HoursDraftRow): RowProblem[] {
  const i = rows.indexOf(row);
  if (i < 0) throw new Error("a row of `rows`");
  const at = `hours[${i}]`;
  const out: RowProblem[] = checkPlaceLive({ hours: hoursOf(rows) })
    .problems.filter((p) => p.path === at || p.path.startsWith(`${at}.`))
    .map((p) => ({ code: p.code }));
  const mine = span(row);
  const clash = new Set<DayOfWeek>();
  for (const other of rows) {
    const theirs = span(other);
    if (other.key === row.key || !mine || !theirs || mine.from >= theirs.to || theirs.from >= mine.to) continue;
    for (const day of row.days) if (other.days.includes(day)) clash.add(day);
  }
  if (clash.size > 0) out.push({ code: "overlap", days: DAYS.filter((d) => clash.has(d)) });
  return out;
}

export function hoursValid(rows: readonly HoursDraftRow[]): boolean {
  return rows.every((r) => rowProblems(rows, r).length === 0);
}

/** The wire rows, days in week order; no rows → undefined, the site's own hours. */
export function hoursOf(rows: readonly HoursDraftRow[]): OpeningHours[] | undefined {
  if (rows.length === 0) return undefined;
  return rows.map((r) => ({ days: DAYS.filter((d) => r.days.includes(d)), opens: r.opens, closes: r.closes }));
}
