import type { Place } from "@/shared/api/schema";

const timeOf = (at: string | null) => (at === null ? Number.NEGATIVE_INFINITY : Date.parse(at));

/**
 * Whether `read` holds a save made after `base` was read. Strictly newer: the
 * re-read after one's own save, or an older answer still on screen while it
 * lands, is not someone else's change.
 */
export function savedSince(read: Pick<Place, "updated_at">, base: Pick<Place, "updated_at">): boolean {
  return timeOf(read.updated_at) > timeOf(base.updated_at);
}
