import type { PricingModel } from "@evinvest/sites-contract";

import type { PricingItem } from "@/entities/pricing";
import { type Failure, api } from "@/shared/api";

export type SaveOutcome =
  | { kind: "saved"; item: PricingItem }
  /** Someone wrote first; `current` is the brand's pricing as it now is (null if it could not be read). */
  | { kind: "conflict"; current: PricingItem | null }
  /** The server's own words: the first problem it found (the editor ran the same check before sending). */
  | { kind: "invalid"; message: string }
  | { kind: "failed"; failure: Failure };

/**
 * One PUT, its refusals sorted into what the editor shows in place. A stale one
 * reads the brand again, so "overwrite" has an `updated_at` to send.
 */
export async function trySave(brand: string, model: PricingModel, expectedUpdatedAt: string | null): Promise<SaveOutcome> {
  const path = { brand };
  const { data, error } = await api().PUT("/pricing/{brand}", { params: { path }, body: { model, expected_updated_at: expectedUpdatedAt } });
  if (error === undefined) return { kind: "saved", item: data };
  if (error.code === "stale") return { kind: "conflict", current: (await api().GET("/pricing/{brand}", { params: { path } })).data ?? null };
  if (error.code === "invalid_pricing") return { kind: "invalid", message: error.error };
  return { kind: "failed", failure: error };
}
