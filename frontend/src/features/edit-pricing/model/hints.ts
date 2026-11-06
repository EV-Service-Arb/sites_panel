import type { PricingInputKind } from "@evinvest/sites-contract";

import { parseHundredths } from "@/entities/pricing";

export type EffectHintKey = `pricing.hint.effect.${PricingInputKind}`;

/**
 * The hint under an answer's effect. An amount's ("0 changes nothing") says
 * something only on an answer that is 0; a multiplier's and a discount's scale
 * is read once, under the first answer.
 */
export function effectHint(kind: PricingInputKind, value: string, first: boolean): EffectHintKey | null {
  if (kind === "add") return parseHundredths(value) === 0 ? "pricing.hint.effect.add" : null;
  return first ? `pricing.hint.effect.${kind}` : null;
}

