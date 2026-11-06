import type { PricingModel } from "@evinvest/sites-contract";

import { type PricingItem, conflictCurrent, fetchPricing, savePricing } from "@/entities/pricing";
import { ApiError } from "@/shared/api";

export type SaveOutcome =
  | { kind: "saved"; item: PricingItem }
  /** Someone wrote first; `current` is the brand's pricing as it now is (null if it could not be read). */
  | { kind: "conflict"; current: PricingItem | null }
  | { kind: "invalid"; path: string; message: string }
  | { kind: "failed"; error: unknown };

/**
 * One PUT, its refusals sorted into what the editor shows in place. A 409
 * normally carries the fresh pricing; if it does not, it is read, so
 * "overwrite" always has an `updated_at` to send.
 */
export async function trySave(brand: string, model: PricingModel, expectedUpdatedAt: string | null): Promise<SaveOutcome> {
  try {
    return { kind: "saved", item: await savePricing(brand, model, expectedUpdatedAt) };
  } catch (e) {
    if (!(e instanceof ApiError)) return { kind: "failed", error: e };
    const f = e.failure;
    if (f.kind === "conflict") return { kind: "conflict", current: conflictCurrent(f.body) ?? (await fetchPricing(brand).catch(() => null)) };
    if (f.kind === "invalid_path") return { kind: "invalid", path: f.path, message: f.message };
    return { kind: "failed", error: e };
  }
}
