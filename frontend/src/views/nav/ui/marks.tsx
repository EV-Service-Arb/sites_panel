import { useResource } from "@evinvest/api/react";
import { NavBadge, NavDot } from "@evinvest/uikit";
import { useEffect, useState } from "react";

import type { Experiment } from "@/entities/experiment";
import { useSelectedBrands } from "@/entities/place";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { tellHost } from "@/shared/lib/host-events";
import { useWindowFocus } from "@/shared/lib/use-window-focus";

import { newLeads, parseSeen, seenAfter, seenKey, seenText, unseen } from "../model/unseen";

const timeOf = (at: string) => Date.parse(at);

/** How many of `times` changed since the screen was last open; nothing until the first answer, which is the baseline. */
function useUnseen(times: readonly number[] | null, open: boolean): number {
  const [seen, setSeen] = useState<number | null>(null);
  // Adjusted during render: the screen open is a fact of this render, and an effect would paint the stale mark once first.
  const next = times === null ? seen : seenAfter(seen, times, open);
  if (next !== seen) setSeen(next);
  return times === null || next === null ? 0 : unseen(times, next);
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
 * New leads since the leads screen was last in front of the person, counted from the total (it only grows), the
 * baseline kept per person across reloads; with none, a dot for bookings without a lead that came since. While the
 * tab is not focused it tells the host the leads that came meanwhile, for the tab title. Keyed by the selection:
 * another one starts the baseline over.
 */
export function LeadsMark({ userId, open }: { userId: string; open: boolean }) {
  const t = useT();
  const focused = useWindowFocus();
  const gmb = gmbParam(useGmbs());
  const selection = gmb ?? "*";
  const counts = useResource(api(), "/leads/counts", { params: { query: { gmb } } });
  const bookings = useResource(api(), "/bookings/unmatched");
  const brands = useSelectedBrands();
  const total = counts.status === "ok" ? counts.data.total : null;
  const [seen, setSeen] = useState<number | null>(() => readSeen(userId, selection));
  const [awayFrom, setAwayFrom] = useState<number | null>(null);
  if (total !== null && (seen === null || seen > total || (open && focused && seen !== total))) setSeen(total);
  if (total !== null && !focused && awayFrom === null) setAwayFrom(total);
  if (focused && awayFrom !== null) setAwayFrom(null);
  useEffect(() => {
    if (seen !== null) writeSeen(userId, seen, selection);
  }, [userId, seen, selection]);

  const away = focused ? 0 : newLeads(total, awayFrom);
  useEffect(() => tellHost("sa:title-count", { count: away }), [away]);
  useEffect(() => () => tellHost("sa:title-count", { count: 0 }), []);

  const selected = brands.status === "ok" ? brands.data : undefined;
  const waiting =
    bookings.status !== "ok" || selected === undefined ? null : bookings.data.bookings.filter((b) => selected === null || selected.has(b.brand)).map((b) => timeOf(b.last_event_at));
  const booked = useUnseen(waiting, open) > 0;
  const leads = newLeads(total, seen);
  if (leads === 0 && booked) return <NavDot label={t("nav.unmatched")} />;
  return <NavBadge count={leads} label={(n) => t("nav.badge", { n })} />;
}

/** The places changed since the places screen was last open: each place once, however many times it changed. */
export function PlacesMark({ open }: { open: boolean }) {
  const t = useT();
  const places = useResource(api(), "/places", { params: { query: { gmb: gmbParam(useGmbs()) } } });
  const times = places.status === "ok" ? places.data.places.flatMap((p) => (p.changed_at === null ? [] : [timeOf(p.changed_at)])) : null;
  return <NavBadge count={useUnseen(times, open)} label={(n) => t("nav.badge", { n })} />;
}

/** When an experiment last changed: its landing's declaration, or an admin's override. */
const changedAt = (e: Experiment) => Math.max(timeOf(e.declared.declared_at), ...(e.override === null ? [] : [timeOf(e.override.changed_at)]), ...(e.weights_changed_at === null ? [] : [timeOf(e.weights_changed_at)]));

/** A dot while an experiment of the selection's brands changed since the experiments screen was last open. */
export function ExperimentsMark({ open, selected }: { open: boolean; selected: ReadonlySet<string> | null }) {
  const t = useT();
  const experiments = useResource(api(), "/experiments");
  const times = experiments.status === "ok" ? experiments.data.experiments.filter((e) => selected === null || selected.has(e.brand)).map(changedAt) : null;
  return useUnseen(times, open) > 0 ? <NavDot label={t("nav.changed")} /> : null;
}

/** The experiments' mark of the selection's brands, which `/places` names. */
export function SelectedExperimentsMark({ open }: { open: boolean }) {
  const brands = useSelectedBrands();
  return brands.status === "ok" ? <ExperimentsMark open={open} selected={brands.data} /> : null;
}
