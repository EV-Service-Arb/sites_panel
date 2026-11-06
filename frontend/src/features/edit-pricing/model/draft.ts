import type { NeedPricing, PricingInputKind, PricingModel } from "@evinvest/sites-contract";

import { hundredthsText, optionValues } from "@/entities/pricing";

export const NEED_KINDS = ["estimate", "fixed"] as const satisfies readonly NeedPricing["kind"][];
export type NeedKind = NeedPricing["kind"];

/**
 * The editor's state: text as typed, so a half-typed "12," is kept rather than
 * lost, and keys of its own so a row keeps its identity (and its errors)
 * while its slug is being renamed. Nothing here is sent: `modelOf` builds the
 * model from it.
 *
 * Amounts are euros ("45.50"); a multiplier and a discount are percents
 * ("110" is ×1.1, "15" is 15 % off).
 */
export interface OptionDraft {
  key: string;
  id: string;
  labels: Record<string, string>;
  value: string;
}

export interface InputDraft {
  key: string;
  id: string;
  kind: PricingInputKind;
  labels: Record<string, string>;
  options: OptionDraft[];
}

export interface NeedDraft {
  key: string;
  id: string;
  kind: NeedKind;
  /** The estimate's base, or the fixed price. */
  amount: string;
  /** The inputs asked, in order, by their draft keys: renaming an input keeps it asked. */
  inputs: string[];
}

export interface PricingDraft {
  validFrom: string;
  roundTo: string;
  minimum: string;
  inputs: InputDraft[];
  needs: NeedDraft[];
}

let counter = 0;
/** A row's identity for the life of the page; never sent. */
export const newKey = (): string => `k${(counter += 1)}`;

/** What a new option starts as: no change to the price. */
const NEUTRAL: Record<PricingInputKind, string> = { add: "0", multiply: "100", discount: "0" };

export function emptyOption(kind: PricingInputKind): OptionDraft {
  return { key: newKey(), id: "", labels: {}, value: NEUTRAL[kind] };
}

export function emptyInput(): InputDraft {
  return { key: newKey(), id: "", kind: "add", labels: {}, options: [emptyOption("add")] };
}

export function emptyNeed(): NeedDraft {
  return { key: newKey(), id: "", kind: "estimate", amount: "", inputs: [] };
}

/** A brand with no model yet: prices from today, rounded to the euro. */
export function emptyDraft(today: string): PricingDraft {
  return { validFrom: today, roundTo: "1", minimum: "0", inputs: [], needs: [] };
}

export function draftOf(model: PricingModel | null, today: string): PricingDraft {
  if (!model) return emptyDraft(today);
  const inputs = model.inputs.map(
    (input): InputDraft => ({
      key: newKey(),
      id: input.id,
      kind: input.kind,
      labels: { ...input.labels },
      options: optionValues(input).map((o) => ({ key: newKey(), id: o.id, labels: { ...o.labels }, value: hundredthsText(o.value) })),
    }),
  );
  const keyOf = (id: string) => inputs.find((i) => i.id === id)?.key;
  const needs = Object.entries(model.needs).map(
    ([id, need]): NeedDraft =>
      need.kind === "fixed"
        ? { key: newKey(), id, kind: "fixed", amount: hundredthsText(need.cents), inputs: [] }
        : { key: newKey(), id, kind: "estimate", amount: hundredthsText(need.baseCents), inputs: need.inputs.map(keyOf).filter((k): k is string => k !== undefined) },
  );
  return { validFrom: model.validFrom, roundTo: hundredthsText(model.roundToCents), minimum: hundredthsText(model.minimumCents), inputs, needs };
}

/**
 * The draft without its keys (an asked input by its slug): two drafts with the
 * same fingerprint say the same thing, whatever rows were added and removed.
 */
export function fingerprint(draft: PricingDraft): string {
  const idOf = (key: string) => draft.inputs.find((i) => i.key === key)?.id ?? key;
  const labels = (l: Record<string, string>) => Object.fromEntries(Object.entries(l).filter(([, v]) => v.trim() !== "").sort(([a], [b]) => a.localeCompare(b)));
  return JSON.stringify({
    validFrom: draft.validFrom,
    roundTo: draft.roundTo.trim(),
    minimum: draft.minimum.trim(),
    inputs: draft.inputs.map((i) => ({ id: i.id, kind: i.kind, labels: labels(i.labels), options: i.options.map((o) => ({ id: o.id, labels: labels(o.labels), value: o.value.trim() })) })),
    needs: draft.needs.map((n) => ({ id: n.id, kind: n.kind, amount: n.amount.trim(), inputs: n.kind === "estimate" ? n.inputs.map(idOf) : [] })),
  });
}
