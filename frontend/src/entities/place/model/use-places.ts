import { type Resource, useResource } from "@evinvest/api/react";

import { type Failure, api } from "@/shared/api";
import type { PlaceSummary } from "@/shared/api/schema";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { useRereadOn } from "@/shared/lib/use-reread-on";

const NONE: PlaceSummary[] = [];

/**
 * The places of the host's selection, to offer in a filter or a form. Until they load, or if they fail,
 * the list is empty: the choice already made stays offered, "Other…" still takes
 * a new location, and the screen's own read reports a backend that is down.
 */
export function usePlaces(version = 0): PlaceSummary[] {
  const places = useResource(api(), "/places", { params: { query: { gmb: gmbParam(useGmbs()) } } });
  useRereadOn(version, places.reload);
  return places.status === "ok" ? places.data.places : NONE;
}

/**
 * The brands the host's selection has a place of, for the lists kept by brand; `null` narrows nothing.
 * Its own read of `/places?gmb=`, so it follows the places' tables and the list beside it follows its own.
 */
export function useSelectedBrands(): Resource<ReadonlySet<string> | null, Failure> {
  const gmbs = useGmbs();
  const places = useResource(api(), "/places", { params: { query: { gmb: gmbParam(gmbs) } } });
  if (places.status !== "ok") return places;
  return { status: "ok", data: gmbs === null ? null : new Set(places.data.places.map((p) => p.brand)) };
}
