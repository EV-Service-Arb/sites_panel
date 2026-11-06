import { type PricingModel, checkPricingModel } from "@evinvest/sites-contract";

import { type Infer, type Parser, ParseError, arrayOf, cents, nullable, object, str } from "@/shared/lib/parse";

/** A model as the API holds it: whole and valid, or the answer is refused. */
export const pricingModelParser: Parser<PricingModel> = (v, path) => {
  const { problems, model } = checkPricingModel(v);
  if (model) return model;
  const first = problems[0];
  throw new ParseError(`${path}: not a pricing model (${first ? `${first.path || "model"}: ${first.code}` : "?"})`);
};

/** One brand's pricing: `GET /pricing/{brand}`, and every write's answer. */
export const pricingItemParser = object({
  brand_id: str,
  /** The brand's site locales: every one needs a label on every input and option. */
  locales: arrayOf(str),
  /** Null: no model in the panel, the site prices from its baked one. */
  model: nullable(pricingModelParser),
  updated_at: nullable(str),
  updated_by: nullable(str),
});
export type PricingItem = Infer<typeof pricingItemParser>;

export const pricingListParser = object({ items: arrayOf(pricingItemParser) });

const changeParser = object({
  id: str,
  at: str,
  by: str,
  /** The model the change left; null when it took the brand's pricing off. */
  model: nullable(pricingModelParser),
});
export type PricingChange = Infer<typeof changeParser>;

/** `GET /pricing/{brand}/changes`, newest first. */
export const pricingChangesParser = object({ changes: arrayOf(changeParser) });

/** `POST /pricing/{brand}/preview`: the price the site would show, or none. */
export const previewParser = object({ cents: nullable(cents) });

/** The 409 of a stale write carries the brand's pricing as it now is. */
export function conflictCurrent(body: unknown): PricingItem | null {
  if (typeof body !== "object" || body === null || !("current" in body)) return null;
  try {
    return pricingItemParser(body.current, "$.current");
  } catch {
    return null;
  }
}
