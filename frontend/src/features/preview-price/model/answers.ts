import type { PricingInputs, PricingModel } from "@evinvest/sites-contract";

/**
 * The answers to send for `need`: the ones picked, where the model still has
 * them, else each asked input's first option — so a price shows at once and
 * survives an option being renamed in the draft.
 */
export function answersFor(model: PricingModel, need: string, picked: PricingInputs): Record<string, string> {
  const pricing = Object.hasOwn(model.needs, need) ? model.needs[need] : undefined;
  if (pricing?.kind !== "estimate") return {};
  const out: Record<string, string> = {};
  for (const id of pricing.inputs) {
    const input = model.inputs.find((i) => i.id === id);
    if (!input) continue;
    const chosen = picked[id];
    const option = input.options.find((o) => o.id === chosen) ?? input.options[0];
    if (option) out[id] = option.id;
  }
  return out;
}

/** The need to preview: the one picked if the model still prices it, else the first it does. */
export function needFor(model: PricingModel, picked: string | null): string | null {
  const needs = Object.keys(model.needs);
  return picked !== null && needs.includes(picked) ? picked : (needs[0] ?? null);
}
