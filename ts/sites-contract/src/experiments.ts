import { validHoldout, validWeights } from "@evinvest/experiments";

import { EXPERIMENT_LIMITS } from "./generated/experiment-limits.js";
import type { LiveExperiments, LiveOverride } from "./generated/types.js";
import type { ExperimentDeclaration } from "./generated/wire.js";
import { charCount, isObject, trim } from "./text.js";

/**
 * The panel's answer to `GET …/brands/{brand}/experiments`, or `null` when it is not one: an
 * override whose field has another type is not the panel's. Fields the panel added since are
 * passed over. Whether an override fits the experiment in code (weights of the right length, a
 * holdout in `[0, 1)`) is `applyOverrides`' call: only it knows the code's variants.
 */
export function parseLiveExperiments(body: unknown): LiveExperiments | null {
  if (!isObject(body) || !isObject(body.experiments)) return null;
  const experiments: Record<string, LiveOverride> = {};
  for (const [key, entry] of Object.entries(body.experiments)) {
    if (!isObject(entry)) return null;
    const { enabled, weights, holdout } = entry;
    if (enabled !== undefined && typeof enabled !== "boolean") return null;
    if (weights !== undefined && !(Array.isArray(weights) && weights.every(w => typeof w === "number"))) return null;
    if (holdout !== undefined && typeof holdout !== "number") return null;
    experiments[key] = {
      ...(enabled !== undefined ? { enabled } : {}),
      ...(weights !== undefined ? { weights } : {}),
      ...(holdout !== undefined ? { holdout } : {}),
    };
  }
  return { experiments };
}

/** Why the panel would refuse an experiment as declared. */
export type DeclarationProblem = "key" | "variants" | "variant" | "duplicate_variant" | "weights" | "holdout" | "summary";

const KEY = /* @__PURE__ */ new RegExp(EXPERIMENT_LIMITS.key);
const VARIANT = /* @__PURE__ */ new RegExp(EXPERIMENT_LIMITS.variant);

/**
 * Why the panel would refuse this experiment of `experiments.declared@1`, or `null`
 * (`contracts/vectors/ingest/declarations.json`): a key; 2 to 32 variants, unique, each a slug;
 * one weight per variant, each ≥ 0, their sum > 0; a holdout in `[0, 1)`; a summary of at most
 * 200 characters once trimmed.
 */
export function declarationProblem(d: ExperimentDeclaration): DeclarationProblem | null {
  if (!KEY.test(d.key)) return "key";
  if (d.variants.length < 2 || d.variants.length > EXPERIMENT_LIMITS.maxVariants) return "variants";
  if (!d.variants.every(v => VARIANT.test(v))) return "variant";
  if (new Set(d.variants).size !== d.variants.length) return "duplicate_variant";
  if (validWeights(d.weights, d.variants.length) === undefined) return "weights";
  if (d.holdout !== undefined && validHoldout(d.holdout) === undefined) return "holdout";
  if (d.summary !== undefined && charCount(trim(d.summary)) > EXPERIMENT_LIMITS.maxSummary) return "summary";
  return null;
}
