import type { PricingInputs, PricingModel } from "@evinvest/sites-contract";
import { useEffect, useState } from "react";

import { previewPrice } from "@/entities/pricing";
import { ApiError } from "@/shared/api";

import { type Runner, type Settled, createRunner } from "./runner";

export const PREVIEW_DEBOUNCE_MS = 300;

export type PreviewState =
  | { kind: "idle" }
  | { kind: "pending" }
  | { kind: "priced"; cents: number | null }
  /** The server refused the model: `path` names the field. */
  | { kind: "invalid"; path: string; message: string }
  | { kind: "failed"; error: unknown };

interface Payload {
  brand: string;
  model: PricingModel;
  need: string;
  answers: PricingInputs;
}

const stateOf = (result: Settled<number | null>): PreviewState => {
  if (result.ok) return { kind: "priced", cents: result.value };
  const e = result.error;
  if (e instanceof ApiError && e.failure.kind === "invalid_path") return { kind: "invalid", path: e.failure.path, message: e.failure.message };
  return { kind: "failed", error: e };
};

/**
 * The server's price for the draft as it stands (`/preview`, the site's own
 * algorithm), asked once typing pauses. The answer is held with the request
 * it answers, so a newer draft shows "pending" rather than an old price.
 */
export function usePreview(brand: string, model: PricingModel | null, need: string | null, answers: PricingInputs): PreviewState {
  const [answered, setAnswered] = useState<{ key: string; state: PreviewState } | null>(null);
  const [runner] = useState<Runner<Payload>>(() =>
    createRunner<Payload, number | null>({
      delayMs: PREVIEW_DEBOUNCE_MS,
      run: (p) => previewPrice(p.brand, p.model, p.need, p.answers),
      onSettled: (key, result) => setAnswered({ key, state: stateOf(result) }),
    }),
  );

  const payload = model && need !== null ? { brand, model, need, answers } : null;
  const key = payload ? JSON.stringify(payload) : null;
  useEffect(() => {
    if (key !== null && payload) runner.request(key, payload);
    return runner.cancel;
    // `key` is the payload, serialised: a new object with the same content asks nothing new.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, runner]);

  if (key === null) return { kind: "idle" };
  return answered?.key === key ? answered.state : { kind: "pending" };
}
