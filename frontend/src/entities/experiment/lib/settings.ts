import type { Experiment } from "../model/experiment";

/** Each variant's share of the traffic in percent; all zero when the weights sum to nothing. */
export function sharesOf(weights: readonly number[]): number[] {
  const sum = weights.reduce((a, b) => a + b, 0);
  return weights.map((w) => (sum > 0 ? (w / sum) * 100 : 0));
}

export function sameWeights(a: readonly number[], b: readonly number[]): boolean {
  return a.length === b.length && a.every((w, i) => w === b[i]);
}

/** No holdout and a holdout of 0 keep the same visitors out: none. */
export function sameHoldout(a: number | null, b: number | null): boolean {
  return (a ?? 0) === (b ?? 0);
}

export type ExperimentStatus = "retired" | "off" | "holdout" | "running";

/** What a row says first; a holdout is only worth saying while the experiment runs. */
export function statusOf(e: Pick<Experiment, "retired" | "effective">): ExperimentStatus {
  if (e.retired) return "retired";
  if (!e.effective.enabled) return "off";
  return (e.effective.holdout ?? 0) > 0 ? "holdout" : "running";
}
