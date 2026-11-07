import { http } from "@/shared/api";
import type { Gmbs } from "@/shared/lib/gmbs";

import { type Funnel, type FunnelByLocation, funnelByLocationParser, funnelParser } from "../model/funnel";

export interface FunnelFilter {
  from: string;
  to: string;
  brand: string | null;
  gmbs: Gmbs;
}

export function fetchFunnel(filter: FunnelFilter): Promise<Funnel> {
  return http.get("/funnel", funnelParser, { from: filter.from, to: filter.to, brand: filter.brand, gmb: filter.gmbs });
}

export function fetchFunnelByLocation(filter: FunnelFilter): Promise<FunnelByLocation> {
  return http.get("/funnel", funnelByLocationParser, { from: filter.from, to: filter.to, brand: filter.brand, gmb: filter.gmbs, by: "location" });
}
