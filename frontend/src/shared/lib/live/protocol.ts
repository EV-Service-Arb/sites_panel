import { type Infer, nullable, object, oneOf, parse, str } from "@/shared/lib/parse";

/** The sites topics; the conductor's own (`sources`, `telegram`) pass by unread. */
export const LIVE_TOPICS = ["leads", "lead", "places", "experiments", "pricing", "bookings", "invoices"] as const;
export type LiveTopic = (typeof LIVE_TOPICS)[number];

const changedParser = object({ topic: oneOf(LIVE_TOPICS), brand_id: nullable(str), id: nullable(str), at: str });
/** Sent after the change has committed, so a read made on it sees the change. */
export type ChangedEvent = Infer<typeof changedParser>;

export type LiveMessage = ({ type: "changed" } & ChangedEvent) | { type: "resync" };

/**
 * A server message the host relayed, or null for anything sites does not read — `hello`, another service's topic,
 * a topic added on the server before this bundle knows it. Ignoring it is safe: the next `resync` (or reconnect)
 * re-reads everything anyway.
 */
export function parseLiveMessage(body: unknown): LiveMessage | null {
  if (typeof body !== "object" || body === null || !("type" in body)) return null;
  if (body.type === "resync") return { type: "resync" };
  if (body.type !== "changed") return null;
  try {
    return { type: "changed", ...parse(changedParser, body) };
  } catch {
    return null;
  }
}

/** The host's socket, as it reports it (`sa:live-status`). */
export const LIVE_STATUSES = ["connecting", "live", "reconnecting", "offline", "paused", "closed"] as const;
export type LiveStatus = (typeof LIVE_STATUSES)[number];
