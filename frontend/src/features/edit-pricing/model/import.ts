import { type PricingModel, type PricingProblem, checkPricingModel } from "@evinvest/sites-contract";

export type ImportResult = { kind: "ok"; model: PricingModel } | { kind: "not_json" } | { kind: "invalid"; problems: PricingProblem[] };

/**
 * A model pasted as JSON — a site's baked one, say — checked as the server
 * will check it, unknown fields refused. Labels missing in a site locale do
 * not stop it: the editor shows them to be filled in before saving.
 */
export function importModel(text: string): ImportResult {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return { kind: "not_json" };
  }
  const { problems, model } = checkPricingModel(value);
  return model ? { kind: "ok", model } : { kind: "invalid", problems };
}

/** The model as it is copied out: the stored shape, indented for a diff. */
export const exportModel = (model: PricingModel): string => `${JSON.stringify(model, null, 2)}\n`;
