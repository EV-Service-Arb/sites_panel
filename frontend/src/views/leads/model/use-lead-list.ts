import { useCallback, useEffect, useRef, useState } from "react";

import { type Lead, type LeadFilter, fetchLeadCard, fetchLeads, refOf } from "@/entities/lead";
import { ApiError, type ApiFailure } from "@/shared/api";
import { useLiveSignal } from "@/shared/lib/live";

import { addWaiting, applyUpdates, arrivals, leadKey, leftFilter, pendingOf, reveal } from "./live-merge";

type ListState =
  | { status: "loading" }
  | { status: "ok"; leads: Lead[]; cursor: string | null; more: boolean }
  | { status: "error"; failure: ApiFailure | { kind: "invalid"; message: string } };

/** How a row was last touched live: arrived ("new"), or changed for the n-th time. */
export type Flash = "new" | number;

const PAGE = 50;
/** Rows that left the filter are re-read one by one; past this many, a burst is better left to the next read. */
const MAX_REREADS = 10;
const NO_FLASH: ReadonlyMap<string, Flash> = new Map();

function failureOf(e: unknown) {
  return e instanceof ApiError ? e.failure : { kind: "invalid" as const, message: e instanceof Error ? e.message : String(e) };
}

/**
 * The queue under a filter, newest first, a page at a time; a new filter,
 * `reload`, or a new `version` (the person's own write) starts over.
 *
 * Live changes never move what the person is looking at: rows on screen are
 * updated where they stand (and flash), arrivals wait in `pending` until
 * `showPending` puts them in. A row that left the filter (a new lead someone
 * contacted) is re-read and kept, showing its new stage.
 */
export function useLeadList(filter: LeadFilter, version = 0) {
  const key = JSON.stringify(filter);
  const [tick, setTick] = useState(0);
  const [state, setState] = useState<{ key: string; tick: number; list: ListState }>({ key, tick, list: { status: "loading" } });
  const [waiting, setWaiting] = useState<{ key: string; leads: Lead[] }>({ key, leads: [] });
  const [flash, setFlash] = useState<{ key: string; marks: ReadonlyMap<string, Flash> }>({ key, marks: NO_FLASH });
  const current = useRef(state);
  useEffect(() => {
    current.current = state;
  });

  useEffect(() => {
    let live = true;
    const f: LeadFilter = JSON.parse(key);
    fetchLeads(f, null, PAGE).then(
      (page) => live && setState({ key, tick, list: { status: "ok", leads: page.leads, cursor: page.next_cursor, more: false } }),
      (e: unknown) => live && setState({ key, tick, list: { status: "error", failure: failureOf(e) } }),
    );
    return () => {
      live = false;
    };
  }, [key, tick, version]);

  // A reload under the same filter keeps the rows on screen until the new page lands.
  const list: ListState = state.key === key ? state.list : { status: "loading" };

  const mark = (k: string, keys: readonly string[], as: "new" | "changed") =>
    setFlash((prev) => {
      const marks = new Map(prev.key === k ? prev.marks : NO_FLASH);
      for (const x of keys) {
        const was = marks.get(x);
        marks.set(x, as === "new" ? "new" : typeof was === "number" ? was + 1 : 1);
      }
      return { key: k, marks };
    });

  const follow = async (k: string) => {
    const f: LeadFilter = JSON.parse(k);
    const page = await fetchLeads(f, null, PAGE).catch(() => null);
    const shownState = current.current;
    // Loading: its own read is on the way. Failed: retrying is the person's call.
    if (!page || shownState.key !== k || shownState.list.status !== "ok") return;
    const shown = shownState.list.leads;
    if (shown.length === 0) {
      // Nothing on screen to keep still: the arrivals go straight in.
      mark(k, page.leads.map(leadKey), "new");
      setState((prev) => (prev.key === k && prev.list.status === "ok" && prev.list.leads.length === 0 ? { ...prev, list: { ...prev.list, leads: page.leads, cursor: page.next_cursor } } : prev));
      return;
    }
    const gone = leftFilter(shown, page.leads, page.next_cursor === null).slice(0, MAX_REREADS);
    const reread = await Promise.all(gone.map((l) => fetchLeadCard(refOf(l)).then((c) => c.lead, () => null)));
    const updates = [...page.leads, ...reread.filter((l) => l !== null)];
    mark(k, applyUpdates(shown, updates).changed, "changed");
    setState((prev) => (prev.key === k && prev.list.status === "ok" ? { ...prev, list: { ...prev.list, leads: applyUpdates(prev.list.leads, updates).leads } } : prev));
    const incoming = arrivals(shown, page.leads);
    if (incoming.length > 0) setWaiting((prev) => ({ key: k, leads: addWaiting(prev.key === k ? prev.leads : [], incoming) }));
  };
  useLiveSignal(["leads", "lead"], () => void follow(key), 400);

  const pending = list.status === "ok" && waiting.key === key ? pendingOf(waiting.leads, list.leads) : [];
  const showPending = () => {
    if (list.status !== "ok" || pending.length === 0) return;
    mark(key, pending.map(leadKey), "new");
    setState({ key, tick, list: { ...list, leads: reveal(list.leads, pending) } });
    setWaiting({ key, leads: [] });
  };

  /** Rejects on failure with the list left as it was; the caller tells the person. */
  const loadMore = async () => {
    if (list.status !== "ok" || !list.cursor || list.more) return;
    setState({ key, tick, list: { ...list, more: true } });
    try {
      const page = await fetchLeads(filter, list.cursor, PAGE);
      setState({ key, tick, list: { status: "ok", leads: [...list.leads, ...page.leads], cursor: page.next_cursor, more: false } });
    } catch (e) {
      setState({ key, tick, list: { ...list, more: false } });
      throw e;
    }
  };

  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { list, loadMore, reload, pending, showPending, flash: flash.key === key ? flash.marks : NO_FLASH };
}
