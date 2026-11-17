import { formatMoment } from "@/shared/lib/format";

import type { Pricing as PricingItem } from "@/shared/api/schema";

/** The brand's last write: who and when, and whether it saved a model or took the pricing off. */
export interface PricingStamp {
  what: "saved" | "cleared";
  at: string;
  by: string;
}

/** Null while nobody has written the brand's pricing from the panel. */
export function pricingStamp(item: Pick<PricingItem, "model" | "updated_at" | "updated_by">, locale: string): PricingStamp | null {
  if (item.updated_at === null) return null;
  return { what: item.model ? "saved" : "cleared", at: formatMoment(item.updated_at, locale), by: item.updated_by ?? "—" };
}
