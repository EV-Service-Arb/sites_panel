/**
 * What changed on a screen since the person last had it open, read off the times its
 * list answers with: the baseline is the newest of them when the screen was last in
 * front of the person, or in the first answer, so no clock but the server's is read.
 */
export function seenAfter(seen: number | null, times: readonly number[], open: boolean): number {
  const newest = Math.max(Number.NEGATIVE_INFINITY, ...times);
  return seen === null || (open && newest > seen) ? newest : seen;
}

/** The items changed after the baseline. */
export function unseen(times: readonly number[], seen: number): number {
  return times.filter((t) => t > seen).length;
}

/**
 * New leads since the leads screen was last in front of the person: leads are
 * never deleted, so the growth of the total is exactly the arrivals. Unknown
 * until both figures are in.
 */
export function newLeads(total: number | null, seen: number | null): number {
  return total === null || seen === null ? 0 : Math.max(0, total - seen);
}

const SEEN_KEY = "panel.leads.seen";

export const seenKey = (userId: string) => `${SEEN_KEY}.${userId}`;

/** The baseline as kept: the count and the selection it was counted under (`gmbParam`, `*` for none). */
export const seenText = (seen: number, selection: string) => `${seen}|${selection}`;

/**
 * localStorage holds text anyone could have written: a count, or nothing. A count of another
 * selection is nothing too: the difference of two sets' totals is no arrival.
 */
export function parseSeen(raw: string | null, selection: string): number | null {
  const m = raw === null ? null : /^(\d{1,9})\|(.*)$/.exec(raw);
  return m === null || m[2] !== selection ? null : Number(m[1]);
}
