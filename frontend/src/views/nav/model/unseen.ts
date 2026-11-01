import type { ChangedEvent } from "@/shared/lib/live";
import type { Route } from "@/shared/lib/router";

/**
 * What changed on a screen since the person last had it open. Places counts the
 * distinct locations changed (two saves of one place are one thing to look at);
 * experiments only say "something".
 */
export interface Unseen {
  places: readonly string[];
  experiments: boolean;
  /** A provider's booking without a lead came or changed: it waits on the leads screen. */
  bookings: boolean;
}

export const NOTHING_UNSEEN: Unseen = { places: [], experiments: false, bookings: false };

const placeOf = (e: ChangedEvent) => (e.id === null ? `${e.brand_id ?? "*"}@${e.at}` : `${e.brand_id ?? "*"}/${e.id}`);

/** Events seen while `at` is open count for nothing on that screen: it is in front of the person. */
export function noteChanges(state: Unseen, events: readonly ChangedEvent[], at: Route | null): Unseen {
  let next = state;
  for (const e of events) {
    if (e.topic === "places" && at !== "places") {
      const key = placeOf(e);
      if (!next.places.includes(key)) next = { ...next, places: [...next.places, key] };
    } else if (e.topic === "experiments" && at !== "experiments" && !next.experiments) next = { ...next, experiments: true };
    else if (e.topic === "bookings" && at !== "leads" && !next.bookings) next = { ...next, bookings: true };
  }
  return next;
}

/** Opening a screen clears its mark. */
export function visit(state: Unseen, at: Route | null): Unseen {
  switch (at) {
    case "places":
      return state.places.length === 0 ? state : { ...state, places: [] };
    case "experiments":
      return state.experiments ? { ...state, experiments: false } : state;
    case "leads":
      return state.bookings ? { ...state, bookings: false } : state;
    default:
      return state;
  }
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

/** localStorage holds text anyone could have written: a count, or nothing. */
export function parseSeen(raw: string | null): number | null {
  if (raw === null || !/^\d{1,9}$/.test(raw)) return null;
  return Number(raw);
}
