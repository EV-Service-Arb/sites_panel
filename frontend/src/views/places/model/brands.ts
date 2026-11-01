import { brandsOf } from "@/entities/place";
import type { Place } from "@/entities/place";
import type { PricingItem } from "@/entities/pricing";

/**
 * The brands a new place may be added under: every place's, and every brand sites lists a price list for —
 * a brand's first place has no place to name the brand yet.
 */
export function offeredBrands(places: readonly Pick<Place, "brand">[], pricing: readonly Pick<PricingItem, "brand_id">[]): string[] {
  return brandsOf([...places, ...pricing.map((p) => ({ brand: p.brand_id }))]);
}
