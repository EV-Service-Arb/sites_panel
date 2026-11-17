import { useLive } from "@evinvest/api/react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Lead, LeadFilter } from "@/entities/lead";
import { type Failure, api } from "@/shared/api";
import { type Gmbs, gmbParam } from "@/shared/lib/gmbs";

import { addWaiting, applyUpdates, arrivals, leadKey, leftFilter, pendingOf, reveal } from "./live-merge";

type ListState =
  | { status: "loading" }
  /** `answer`: the first page's, whose tables the list follows. */
  | { status: "ok"; leads: Lead[]; cursor: string | null; more: boolean; answer: Response }
  | { status: "error"; failure: Failure };

/** How a row was last touched live: arrived ("new"), or changed for the n-th time. */
export type Flash = "new" | number;

const PAGE = 50;
/** Rows that left the filter are re-read one by one; past this many, a burst is better left to the next read. */
const MAX_REREADS = 10;
const NO_FLASH: ReadonlyMap<string, Flash> = new Map();

const page = (filter: LeadFilter, gmbs: Gmbs, cursor: string | null) => api().GET("/leads", { params: { query: { ...filter, gmb: gmbParam(gmbs), cursor, limit: PAGE } } });

/**
 * The queue under a filter, newest first, a page at a time; a new filter,
 * `reload`, or a new `version` (the person's own write) starts over.
 *
 * Live changes never move what the person is looking at: rows on screen are
 * updated where they stand (and flash), arrivals wait in `pending` until
 * `showPending` puts them in. A row that left the filter (a new lead someone
 * contacted) is re-read and kept, showing its new stage.
 */
export function useLeadList(filter: LeadFilter, gmbs: Gmbs, version = 0) {
  const key = JSON.stringify([filter, gmbs]);
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
    const [f, g]: [LeadFilter, Gmbs] = JSON.parse(key);
    page(f, g, null).then(
      ({ data, error, response }) =>
        live &&
        setState({
          key,
          tick,
          list: error === undefined ? { status: "ok", leads: data.leads, cursor: data.next_cursor, more: false, answer: response } : { status: "error", failure: error },
        }),
      // A broken contract (an answer off the document): thrown in render, as a read's is.
      (broken: unknown) =>
        live &&
        setState(() => {
          throw broken;
        }),
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
    const [f, g]: [LeadFilter, Gmbs] = JSON.parse(k);
    const { data: fresh } = await page(f, g, null);
    const shownState = current.current;
    // Refused: a re-read nobody asked for stays quiet, and retrying is the person's call. Loading: its own read is on the way.
    if (fresh === undefined || shownState.key !== k || shownState.list.status !== "ok") return;
    const shown = shownState.list.leads;
    if (shown.length === 0) {
      // Nothing on screen to keep still: the arrivals go straight in.
      mark(k, fresh.leads.map(leadKey), "new");
      setState((prev) => (prev.key === k && prev.list.status === "ok" && prev.list.leads.length === 0 ? { ...prev, list: { ...prev.list, leads: fresh.leads, cursor: fresh.next_cursor } } : prev));
      return;
    }
    const gone = leftFilter(shown, fresh.leads, fresh.next_cursor === null).slice(0, MAX_REREADS);
    // A lead that cannot be read again stays as it was shown: the next change re-reads it.
    const reread = await Promise.all(gone.map(async (l) => (await api().GET("/leads/{brand}/{lead}", { params: { path: { brand: l.brand, lead: l.lead_id } } })).data?.lead));
    const updates = [...fresh.leads, ...reread.filter((l) => l !== undefined)];
    mark(k, applyUpdates(shown, updates).changed, "changed");
    setState((prev) => (prev.key === k && prev.list.status === "ok" ? { ...prev, list: { ...prev.list, leads: applyUpdates(prev.list.leads, updates).leads } } : prev));
    const incoming = arrivals(shown, fresh.leads);
    if (incoming.length > 0) setWaiting((prev) => ({ key: k, leads: addWaiting(prev.key === k ? prev.leads : [], incoming) }));
  };
  useLive(list.status === "ok" ? list.answer : undefined, () => void follow(key));

  const pending = list.status === "ok" && waiting.key === key ? pendingOf(waiting.leads, list.leads) : [];
  const showPending = () => {
    if (list.status !== "ok" || pending.length === 0) return;
    mark(key, pending.map(leadKey), "new");
    setState({ key, tick, list: { ...list, leads: reveal(list.leads, pending) } });
    setWaiting({ key, leads: [] });
  };

  /** The refusal, with the list left as it was; the caller tells the person. */
  const loadMore = async (): Promise<Failure | null> => {
    if (list.status !== "ok" || !list.cursor || list.more) return null;
    setState({ key, tick, list: { ...list, more: true } });
    const { data, error } = await page(filter, gmbs, list.cursor);
    if (error !== undefined) {
      setState({ key, tick, list: { ...list, more: false } });
      return error;
    }
    setState({ key, tick, list: { ...list, leads: [...list.leads, ...data.leads], cursor: data.next_cursor, more: false } });
    return null;
  };

  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { list, loadMore, reload, pending, showPending, flash: flash.key === key ? flash.marks : NO_FLASH };
}
