import { http } from "@/shared/api";
import type { Gmbs } from "@/shared/lib/gmbs";

import { type Place, brandsOf, placesParser } from "../model/place";

/** The places of the host's selection (every place for `null`). */
export async function fetchPlaces(gmbs: Gmbs): Promise<Place[]> {
  return (await http.get("/places", placesParser, { gmb: gmbs })).places;
}

/** The brands the selection has a place of, for the lists kept by brand; `null` keeps every brand. */
export async function selectedBrands(gmbs: Gmbs): Promise<ReadonlySet<string> | null> {
  return gmbs === null ? null : new Set(brandsOf(await fetchPlaces(gmbs)));
}
