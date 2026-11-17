const timeOf = (at: string | null) => (at === null ? Number.NEGATIVE_INFINITY : Date.parse(at));

/**
 * Whether `read` holds a write made after `base` was read: someone else's,
 * since one's own re-pins. Strictly newer, so an older answer still on screen
 * while a re-read lands is not mistaken for one (a removal is a write too: it
 * keeps an `updated_at`).
 */
export function pricingSavedSince(read: { updated_at: string | null }, base: { updated_at: string | null }): boolean {
  return timeOf(read.updated_at) > timeOf(base.updated_at);
}
