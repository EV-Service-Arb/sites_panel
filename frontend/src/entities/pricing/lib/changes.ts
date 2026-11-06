import type { PricingModel } from "@evinvest/sites-contract";

/** The parts of a model a history row names when it says what a save changed. */
export const MODEL_PARTS = ["validFrom", "general", "inputs", "needs"] as const;
export type ModelPart = (typeof MODEL_PARTS)[number];

/** JSON with object keys sorted: a record read back in another order is the same record. */
function canonical(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (typeof v === "object" && v !== null) {
    const entries = Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    return `{${entries.map(([k, x]) => `${JSON.stringify(k)}:${canonical(x)}`).join(",")}}`;
  }
  return JSON.stringify(v);
}

const partOf: Record<ModelPart, (m: PricingModel) => unknown> = {
  validFrom: (m) => m.validFrom,
  general: (m) => [m.roundToCents, m.minimumCents],
  inputs: (m) => m.inputs,
  needs: (m) => m.needs,
};

/**
 * What a save changed against the model before it, in the model's order; empty
 * for the same model saved again. Null when there is nothing to compare: the
 * first save, or either side a take-off.
 */
export function changedParts(model: PricingModel | null, before: PricingModel | null | undefined): ModelPart[] | null {
  if (!model || !before) return null;
  return MODEL_PARTS.filter((p) => canonical(partOf[p](model)) !== canonical(partOf[p](before)));
}
