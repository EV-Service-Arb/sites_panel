import type { PricingInputKind } from "./generated/input-kinds.js";
import type { PricingInput, PricingModel } from "./generated/types.js";

/** The answers to an estimate's inputs: input id → option id. */
export type PricingInputs = Readonly<Record<string, string>>;

/** The labels of an input or an option, by locale. */
export type PricingLabels = PricingInput["labels"];

/** The effect of each option of an input, in the unit its kind says. */
export function effects(input: PricingInput): { id: string; effect: number }[] {
  switch (input.kind) {
    case "add":
      return input.options.map(o => ({ id: o.id, effect: o.addCents }));
    case "multiply":
      return input.options.map(o => ({ id: o.id, effect: o.multiplyBp }));
    case "discount":
      return input.options.map(o => ({ id: o.id, effect: o.discountBp }));
  }
}

const BP = 10_000;

/**
 * `cents × bp / 10 000`, rounded half up to the cent. Integers throughout: the product is exact
 * (the model's limits keep it under 2^53), and the division is done on the remainder.
 */
export function mulBp(cents: number, bp: number): number {
  const n = cents * bp + BP / 2;
  return (n - (n % BP)) / BP;
}

/** Half up to a multiple of `step` (≥ 1). */
export function roundTo(cents: number, step: number): number {
  const rest = cents % step;
  return rest * 2 >= step ? cents - rest + step : cents - rest;
}

/**
 * One line of how a price was reached; `cents` is what the line changed, so the lines add up to
 * the total. `base` is the need's base (or the fixed price); `rounding` and `minimum` appear only
 * when they changed something.
 */
export type PriceLine =
  | { kind: "base"; cents: number }
  | { kind: PricingInputKind; input: string; option: string; cents: number }
  | { kind: "rounding"; cents: number }
  | { kind: "minimum"; cents: number };

export interface Price {
  cents: number;
  breakdown: readonly PriceLine[];
}

/**
 * What a need costs for these answers (`contracts/vectors/pricing/README.md`, normative):
 *
 * 1. the need's base, plus every `add` input, in the need's input order;
 * 2. times every `multiply` input, in order, each product rounded half up to the cent at once;
 * 3. every `discount` input, in order, as a multiplication by `10 000 − discountBp`, rounded the
 *    same way;
 * 4. the total rounded half up to a multiple of `roundToCents`;
 * 5. raised to `minimumCents`.
 *
 * A `fixed` need is its price, as is. `null`: the need is not priced, or an input is unanswered
 * or answered with an option it does not have — never a guess. Answers to inputs the need does
 * not ask are ignored.
 */
export function priceOf(model: PricingModel, need: string, inputs: PricingInputs): Price | null {
  const pricing = Object.hasOwn(model.needs, need) ? model.needs[need] : undefined;
  if (!pricing) return null;
  if (pricing.kind === "fixed") return { cents: pricing.cents, breakdown: [{ kind: "base", cents: pricing.cents }] };
  const answers: { input: PricingInput; option: string; effect: number }[] = [];
  for (const id of pricing.inputs) {
    const input = model.inputs.find(i => i.id === id);
    const answer = Object.hasOwn(inputs, id) ? inputs[id] : undefined;
    const option = input && effects(input).find(o => o.id === answer);
    if (!input || !option) return null;
    answers.push({ input, option: option.id, effect: option.effect });
  }
  const lines: PriceLine[] = [{ kind: "base", cents: pricing.baseCents }];
  let total = pricing.baseCents;
  const step = (kind: PricingInputKind, apply: (total: number, effect: number) => number) => {
    for (const a of answers) {
      if (a.input.kind !== kind) continue;
      const next = apply(total, a.effect);
      lines.push({ kind, input: a.input.id, option: a.option, cents: next - total });
      total = next;
    }
  };
  step("add", (t, cents) => t + cents);
  step("multiply", (t, bp) => mulBp(t, bp));
  step("discount", (t, bp) => mulBp(t, BP - bp));
  const rounded = roundTo(total, model.roundToCents);
  if (rounded !== total) lines.push({ kind: "rounding", cents: rounded - total });
  total = rounded;
  if (total < model.minimumCents) {
    lines.push({ kind: "minimum", cents: model.minimumCents - total });
    total = model.minimumCents;
  }
  return { cents: total, breakdown: lines };
}
