import { useEffect, useState } from "react";

import { fetchLeadCounts } from "@/entities/lead";
import { usePlaces } from "@/entities/place";
import { gmbsKey, useGmbs } from "@/shared/lib/gmbs";
import { type ChangedEvent, useLiveSignal } from "@/shared/lib/live";
import type { Route } from "@/shared/lib/router";
import { useWindowFocus } from "@/shared/lib/use-window-focus";

import { NOTHING_UNSEEN, type Unseen, newLeads, noteChanges, parseSeen, seenKey, seenText, visit } from "./unseen";

export interface Marks {
  /** New leads since the leads screen was last in front of the person. */
  leads: number;
  /** New leads since this tab lost focus: the tab title's count. */
  away: number;
  places: number;
  experiments: boolean;
  /** Bookings without a lead came since the leads screen was last open. */
  bookings: boolean;
}

function readSeen(userId: string, selection: string): number | null {
  try {
    return parseSeen(window.localStorage.getItem(seenKey(userId)), selection);
  } catch {
    // Storage refused (a private window): the count starts from this visit.
    return null;
  }
}

function writeSeen(userId: string, seen: number, selection: string): void {
  try {
    window.localStorage.setItem(seenKey(userId), seenText(seen, selection));
  } catch {
    // As above: kept for this page's life only.
  }
}

/**
 * The nav's marks. Leads are counted from the total (it only grows), the
 * baseline kept per person across reloads; places and experiments from the
 * live events since their screen was last open, for this page's life. All of
 * them of the host's selection: another selection starts the leads' baseline over.
 * `countsLeads`: the caller may read the counts (Work); without it Leads is never marked.
 */
export function useMarks(userId: string, at: Route | null, countsLeads: boolean): Marks {
  const focused = useWindowFocus();
  const gmbs = useGmbs();
  const selection = gmbsKey(gmbs);
  const places = usePlaces();
  const [unseen, setUnseen] = useState<Unseen>(NOTHING_UNSEEN);
  const [total, setTotal] = useState<number | null>(null);
  const [seen, setSeen] = useState<number | null>(() => readSeen(userId, selection));
  const [awayFrom, setAwayFrom] = useState<number | null>(null);
  const [countedFor, setCountedFor] = useState(selection);

  // Adjusted during render rather than in effects: each is a fact about this
  // render (the screen open, the window focused), and an effect would paint the
  // stale mark once first.
  if (countedFor !== selection) {
    setCountedFor(selection);
    setTotal(null);
    setSeen(readSeen(userId, selection));
    setAwayFrom(null);
  }
  const visited = visit(unseen, at);
  if (visited !== unseen) setUnseen(visited);
  if (total !== null && (seen === null || seen > total || (at === "leads" && focused && seen !== total))) setSeen(total);
  if (total !== null && !focused && awayFrom === null) setAwayFrom(total);
  if (focused && awayFrom !== null) setAwayFrom(null);

  useEffect(() => {
    if (seen !== null) writeSeen(userId, seen, countedFor);
  }, [userId, seen, countedFor]);

  // Re-counted on every lead change; an answer overtaken by a newer count is dropped.
  const [recount, setRecount] = useState(0);
  useEffect(() => {
    if (!countsLeads) return;
    let live = true;
    fetchLeadCounts({ brand: null, gmbs }).then(
      (counts) => live && setTotal(counts.total),
      // A mark is a hint: a failed count keeps the last one rather than shouting.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [recount, countsLeads, gmbs]);
  useLiveSignal(["leads", "lead"], () => setRecount((n) => n + 1), 400);
  useLiveSignal(["places", "experiments", "bookings"], (signals) => {
    const selected = (e: ChangedEvent) =>
      gmbs === null || (e.topic === "places" ? places.some((p) => p.brand === e.brand_id && p.location === e.id) : places.some((p) => p.brand === e.brand_id));
    const events = signals.flatMap((s): ChangedEvent[] => (s.kind === "changed" && selected(s.event) ? [s.event] : []));
    if (events.length > 0) setUnseen((u) => noteChanges(u, events, at));
  });

  return {
    leads: newLeads(total, seen),
    away: focused ? 0 : newLeads(total, awayFrom),
    places: unseen.places.length,
    experiments: unseen.experiments,
    bookings: unseen.bookings,
  };
}
