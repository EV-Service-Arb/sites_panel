import type { PricingInputs, PricingModel } from "@evinvest/sites-contract";
import { useEffect, useState } from "react";

import { type Failure, api } from "@/shared/api";

import { type Runner, createRunner } from "./runner";

export const PREVIEW_DEBOUNCE_MS = 300;

export type PreviewState =
  | { kind: "idle" }
  | { kind: "pending" }
  | { kind: "priced"; cents: number | null }
  /** The server refused the model, in its own words. */
  | { kind: "invalid"; message: string }
  | { kind: "failed"; failure: Failure };

type Answered = Exclude<PreviewState, { kind: "idle" } | { kind: "pending" }>;

interface Payload {
  brand: string;
  model: PricingModel;
  need: string;
  answers: PricingInputs;
}

/** The server prices `model` as the site would: the editor never computes a price itself. */
async function preview({ brand, model, need, answers }: Payload): Promise<Answered> {
  const { data, error } = await api().POST("/pricing/{brand}/preview", { params: { path: { brand } }, body: { model, need, inputs: answers } });
  if (error === undefined) return { kind: "priced", cents: data.cents };
  if (error.code === "invalid_pricing") return { kind: "invalid", message: error.error };
  return { kind: "failed", failure: error };
}

/**
 * The server's price for the draft as it stands (`/preview`, the site's own
 * algorithm), asked once typing pauses. The answer is held with the request
 * it answers, so a newer draft shows "pending" rather than an old price.
 */
export function usePreview(brand: string, model: PricingModel | null, need: string | null, answers: PricingInputs): PreviewState {
  const [answered, setAnswered] = useState<{ key: string; state: PreviewState } | null>(null);
  const [runner] = useState<Runner<Payload>>(() =>
    createRunner<Payload, Answered>({
      delayMs: PREVIEW_DEBOUNCE_MS,
      run: preview,
      // A rejection is a broken contract (an answer off the document): thrown in render, as a read's is.
      onSettled: (key, result) =>
        setAnswered(() => {
          if (!result.ok) throw result.error;
          return { key, state: result.value };
        }),
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
