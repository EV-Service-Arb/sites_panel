import type { PricingItem } from "@/entities/pricing";
import { type Failure, api } from "@/shared/api";

export type ClearOutcome =
  | { kind: "cleared"; item: PricingItem }
  /** Someone wrote first; `current` is the brand's pricing as it now is (null if it could not be read). */
  | { kind: "conflict"; current: PricingItem | null }
  | { kind: "failed"; failure: Failure };

/**
 * One DELETE, its answer kept: the screen pins what the server now holds
 * instead of reading it again, so its own removal is never taken for someone else's.
 */
export async function tryClear(item: PricingItem): Promise<ClearOutcome> {
  if (item.updated_at === null) throw new Error(`${item.brand_id}: a model is there only once saved, and a save stamps it`);
  const path = { brand: item.brand_id };
  const { data, error } = await api().DELETE("/pricing/{brand}", { params: { path }, body: { expected_updated_at: item.updated_at } });
  if (error === undefined) return { kind: "cleared", item: data };
  if (error.code === "stale") return { kind: "conflict", current: (await api().GET("/pricing/{brand}", { params: { path } })).data ?? null };
  return { kind: "failed", failure: error };
}
