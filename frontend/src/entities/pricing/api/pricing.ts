import { http } from "@/shared/api";

import { type PricingChange, type PricingItem, pricingChangesParser, pricingItemParser, pricingListParser, previewParser } from "../model/item";
import type { PricingAnswers, PricingModel } from "../model/model";

const brandPath = (brand: string) => `/pricing/${encodeURIComponent(brand)}`;

export async function fetchPricingList(): Promise<PricingItem[]> {
  return (await http.get("/pricing", pricingListParser)).items;
}

export function fetchPricing(brand: string): Promise<PricingItem> {
  return http.get(brandPath(brand), pricingItemParser);
}

/**
 * Admin only; a full replace. `expectedUpdatedAt` is the `updated_at` the
 * draft started from: a newer one answers 409 with the pricing as it now is.
 */
export function savePricing(brand: string, model: PricingModel, expectedUpdatedAt: string | null): Promise<PricingItem> {
  return http.send("PUT", brandPath(brand), { model, expected_updated_at: expectedUpdatedAt }, pricingItemParser);
}

/**
 * Admin only: the brand's sites go back to their baked prices. Guarded like a
 * save, and answered like one: the pricing as it now is (no model, the removal's stamp).
 */
export function clearPricing(brand: string, expectedUpdatedAt: string | null): Promise<PricingItem> {
  return http.send("DELETE", brandPath(brand), { expected_updated_at: expectedUpdatedAt }, pricingItemParser);
}

export async function fetchPricingChanges(brand: string): Promise<PricingChange[]> {
  return (await http.get(`${brandPath(brand)}/changes`, pricingChangesParser)).changes;
}

/** The server prices `model` as the site would: the editor never computes a price itself. */
export async function previewPrice(brand: string, model: PricingModel, need: string, inputs: PricingAnswers): Promise<number | null> {
  return (await http.send("POST", `${brandPath(brand)}/preview`, { model, need, inputs }, previewParser)).cents;
}
