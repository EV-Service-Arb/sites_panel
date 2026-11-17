import type { PlaceSummary as Place } from "@/shared/api/schema";

const uniq = (xs: Iterable<string>) => [...new Set(xs)].sort();

/** The brands to offer, plus the one already chosen even if no place names it. */
export function brandsOf(places: readonly Pick<Place, "brand">[], chosen: string | null = null): string[] {
  return uniq([...places.map((p) => p.brand), ...(chosen ? [chosen] : [])]);
}

/** A brand's locations (every brand's when none is chosen), plus the chosen one. */
export function locationsOf(places: readonly Pick<Place, "brand" | "location">[], brand: string | null, chosen: string | null = null): string[] {
  return uniq([...places.filter((p) => !brand || p.brand === brand).map((p) => p.location), ...(chosen ? [chosen] : [])]);
}
