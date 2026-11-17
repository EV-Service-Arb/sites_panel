import type { FunnelStep, LocationSlice } from "@/entities/funnel";
import type { Place } from "@/entities/place";
import type { Share } from "@/shared/lib/share";

/** The steps a location's card shows, each as a share of its leads. */
export const PLACE_STEPS = ["contacted", "won", "paid"] as const satisfies readonly FunnelStep["stage"][];
export type PlaceStep = (typeof PLACE_STEPS)[number];

export interface PlaceRow {
  brand: string;
  location: string | null;
  leads: number;
  steps: { stage: PlaceStep; share: Share }[];
  /** What `/places` says of the location's site data; null for the leads that name none. */
  site: { hasSettings: boolean; withdrawn: boolean } | null;
}

const keyOf = (brand: string, location: string) => `${brand}/${location}`;

/**
 * A card per location of `/funnel?by=location`, busiest first, and one for each
 * place `/places` knows with no lead in the window (added by hand, say). The
 * shares are the backend's (`of_leads`), percent withheld below its minimum
 * sample — none is counted here.
 */
export function placeRows(slices: readonly LocationSlice[], places: readonly Place[] = []): PlaceRow[] {
  const known = new Map(places.map((p) => [keyOf(p.brand, p.location), { hasSettings: p.has_settings, withdrawn: p.withdrawn }]));
  const site = (brand: string, location: string | null) => (location === null ? null : (known.get(keyOf(brand, location)) ?? { hasSettings: false, withdrawn: false }));
  const seen = new Set(slices.map((s) => keyOf(s.brand, s.location ?? "")));
  const quiet: PlaceRow[] = places
    .filter((p) => !seen.has(keyOf(p.brand, p.location)))
    .map((p) => ({ brand: p.brand, location: p.location, leads: 0, steps: [], site: site(p.brand, p.location) }));
  const rows = slices.map((s) => ({
    brand: s.brand,
    location: s.location,
    leads: s.stages.find((step) => step.stage === "created")?.reached ?? 0,
    steps: PLACE_STEPS.flatMap((stage) => {
      const step = s.stages.find((x) => x.stage === stage);
      return step ? [{ stage, share: step.of_leads }] : [];
    }),
    site: site(s.brand, s.location),
  }));
  return [...rows, ...quiet].sort((a, b) => b.leads - a.leads || a.brand.localeCompare(b.brand) || (a.location ?? "").localeCompare(b.location ?? ""));
}
