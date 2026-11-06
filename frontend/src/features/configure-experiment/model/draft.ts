import { validHoldout, validWeights } from "@evinvest/sites-contract";

import { type Experiment, type ExperimentPatch, sameHoldout, sameWeights } from "@/entities/experiment";

/** The form as typed: a weight per variant, and the holdout in percent ("" for none). */
export interface Draft {
  weights: string[];
  holdout: string;
}

/** "1", "0.5", "0,5" → a number; anything else (signs and exponents included) → null. */
export function parseDecimal(raw: string): number | null {
  const s = raw.trim().replace(",", ".");
  return /^\d+(\.\d+)?$/.test(s) ? Number(s) : null;
}

/** A fraction as the percent a person types, without float dust (0.07 → "7", not "7.000000000000001"). */
function percentText(fraction: number): string {
  return String(Number((fraction * 100).toFixed(4)));
}

export function draftOf(e: Experiment): Draft {
  const h = e.effective.holdout;
  return { weights: e.effective.weights.map(String), holdout: h === null || h === 0 ? "" : percentText(h) };
}

export type DraftErrors = { weights?: true; holdout?: true };

export type DraftCheck =
  | { kind: "invalid"; errors: DraftErrors }
  | { kind: "unchanged" }
  /** `weightsChanged`: PostHog's comparison must be read from now on, and the person is told so. */
  | { kind: "ready"; patch: ExperimentPatch; weightsChanged: boolean };

/**
 * The PUT a draft makes, checked by the rules a landing applies it by. Only
 * what differs from the effective setting is sent, and a value equal to the
 * code's is sent as null: the override then holds only real deviations, and a
 * later change in code is not masked by a copy of the old value.
 */
export function checkDraft(e: Experiment, d: Draft): DraftCheck {
  const parsed = d.weights.map(parseDecimal);
  const weights = parsed.every((w): w is number => w !== null) && validWeights(parsed, e.variants.length) !== undefined ? parsed : null;
  const holdoutRaw = d.holdout.trim();
  const pct = holdoutRaw === "" ? 0 : parseDecimal(holdoutRaw);
  const holdout = pct !== null && validHoldout(pct / 100) !== undefined ? (pct === 0 ? null : pct / 100) : undefined;

  const errors: DraftErrors = {};
  if (weights === null) errors.weights = true;
  if (holdout === undefined) errors.holdout = true;
  if (weights === null || holdout === undefined) return { kind: "invalid", errors };

  const patch: ExperimentPatch = {};
  const weightsChanged = !sameWeights(weights, e.effective.weights);
  if (weightsChanged) patch.weights = sameWeights(weights, e.declared.weights) ? null : weights;
  if (!sameHoldout(holdout, e.effective.holdout)) patch.holdout = sameHoldout(holdout, e.declared.holdout) ? null : (holdout ?? 0);
  return Object.keys(patch).length === 0 ? { kind: "unchanged" } : { kind: "ready", patch, weightsChanged };
}

/** The kill switch: back to the code's value when that is what is chosen. */
export function enabledPatch(e: Experiment, enabled: boolean): ExperimentPatch {
  return { enabled: enabled === e.declared.enabled ? null : enabled };
}

/** "As in code": every override dropped. */
export const RESET_PATCH: ExperimentPatch = { enabled: null, weights: null, holdout: null };

/** Whether dropping the override moves traffic between variants. */
export function resetMovesWeights(e: Experiment): boolean {
  return !sameWeights(e.effective.weights, e.declared.weights);
}
