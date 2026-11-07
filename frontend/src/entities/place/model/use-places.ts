import { gmbsKey, useGmbs } from "@/shared/lib/gmbs";
import { useResource } from "@/shared/lib/use-resource";

import { fetchPlaces } from "../api/places";
import type { Place } from "./place";

const NONE: Place[] = [];

/**
 * The places of the host's selection, to offer in a filter or a form. Until they load, or if they fail,
 * the list is empty: the choice already made stays offered, "Other…" still takes
 * a new location, and the screen's own read reports a backend that is down.
 */
export function usePlaces(version = 0): Place[] {
  const gmbs = useGmbs();
  const key = `places:${gmbsKey(gmbs)}`;
  const places = useResource(`${key}:${version}`, () => fetchPlaces(gmbs), key, { live: ["places"] });
  return places.status === "ok" ? places.data : NONE;
}
