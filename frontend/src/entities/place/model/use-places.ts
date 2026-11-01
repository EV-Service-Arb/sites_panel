import { useResource } from "@/shared/lib/use-resource";

import { fetchPlaces } from "../api/places";
import type { Place } from "./place";

const NONE: Place[] = [];

/**
 * The places to offer in a filter or a form. Until they load, or if they fail,
 * the list is empty: the choice already made stays offered, "Other…" still takes
 * a new location, and the screen's own read reports a backend that is down.
 */
export function usePlaces(version = 0): Place[] {
  const places = useResource(`places:${version}`, fetchPlaces, "places", { live: ["places"] });
  return places.status === "ok" ? places.data : NONE;
}
