/**
 * A brand's price list as data, the shape kitstart reads (EV-Service-Arb/sites_tools
 * `ts/kitstart/src/core/pricing/model.ts`, as of EV-invest/lib b7ef642). The panel stores, edits
 * and serves exactly this object: the site and the server refuse a model with
 * a field they do not know, so nothing of the editor's own rides along.
 *
 * Money is integer euro cents, TTC; a multiplier or a discount is in basis
 * points (10 000 = ×1, 1 000 = 10 %).
 */

export const PRICING_FORMAT = 1;
export const PRICING_CURRENCY = "EUR";

/** A label per locale (`{ fr: "Studio", en: "Studio" }`). */
export type PricingLabels = Readonly<Record<string, string>>;

export interface AddOption {
  id: string;
  labels: PricingLabels;
  addCents: number;
}

export interface MultiplyOption {
  id: string;
  labels: PricingLabels;
  multiplyBp: number;
}

export interface DiscountOption {
  id: string;
  labels: PricingLabels;
  discountBp: number;
}

export type PricingInput =
  | { id: string; kind: "add"; labels: PricingLabels; options: readonly AddOption[] }
  | { id: string; kind: "multiply"; labels: PricingLabels; options: readonly MultiplyOption[] }
  | { id: string; kind: "discount"; labels: PricingLabels; options: readonly DiscountOption[] };

export const INPUT_KINDS = ["add", "multiply", "discount"] as const;
export type PricingInputKind = (typeof INPUT_KINDS)[number];

/** The field an option of each kind carries its effect in. */
export const EFFECT_FIELD = { add: "addCents", multiply: "multiplyBp", discount: "discountBp" } as const satisfies Record<PricingInputKind, string>;

export const NEED_KINDS = ["estimate", "fixed"] as const;
export type NeedKind = (typeof NEED_KINDS)[number];

export type NeedPricing = { kind: "estimate"; baseCents: number; inputs: readonly string[] } | { kind: "fixed"; cents: number };

export interface PricingModel {
  format: typeof PRICING_FORMAT;
  currency: typeof PRICING_CURRENCY;
  /** `YYYY-MM-DD`; stored with every lead priced by the model. */
  validFrom: string;
  /** The estimate's total is rounded half up to a multiple of this. */
  roundToCents: number;
  /** An estimate never comes out below this. */
  minimumCents: number;
  inputs: readonly PricingInput[];
  /** By the brand's need slug; a need not here is a quote. */
  needs: Readonly<Record<string, NeedPricing>>;
}

/** The answers to an estimate's inputs: input id → option id. */
export type PricingAnswers = Readonly<Record<string, string>>;

/** kitstart's `PRICING_LIMITS`, the same numbers. */
export const PRICING_LIMITS = {
  slug: /^[a-z0-9_-]{1,40}$/,
  locale: /^[a-z]{2}(-[A-Z]{2})?$/,
  maxLabel: 120,
  maxInputs: 32,
  maxOptions: 32,
  maxNeeds: 64,
  maxNeedInputs: 12,
  maxPriceCents: 100_000_000,
  maxMultiplyBp: 100_000,
  maxDiscountBp: 10_000,
} as const;

export const EFFECT_MAX = {
  add: PRICING_LIMITS.maxPriceCents,
  multiply: PRICING_LIMITS.maxMultiplyBp,
  discount: PRICING_LIMITS.maxDiscountBp,
} as const satisfies Record<PricingInputKind, number>;
