import { http } from "@/shared/api";

import { type Funnel, type FunnelByLocation, funnelByLocationParser, funnelParser } from "../model/funnel";

export interface FunnelFilter {
  from: string;
  to: string;
  brand: string | null;
}

export function fetchFunnel(filter: FunnelFilter): Promise<Funnel> {
  return http.get("/funnel", funnelParser, { from: filter.from, to: filter.to, brand: filter.brand });
}

export function fetchFunnelByLocation(filter: FunnelFilter): Promise<FunnelByLocation> {
  return http.get("/funnel", funnelByLocationParser, { from: filter.from, to: filter.to, brand: filter.brand, by: "location" });
}
