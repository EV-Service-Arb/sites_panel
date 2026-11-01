import { http } from "@/shared/api";

import { type Place, placesParser } from "../model/place";

export async function fetchPlaces(): Promise<Place[]> {
  return (await http.get("/places", placesParser)).places;
}
