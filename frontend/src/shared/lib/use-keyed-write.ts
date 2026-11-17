import { useRef, useState } from "react";

import { type Attempt, type Failure, attemptFor } from "@/shared/api";

export type WriteOutcome = { ok: true } | { ok: false; failure: Failure };

/**
 * One write at a time under an Idempotency-Key. The key stays with its
 * attempt: the same body retried after a lost answer goes with the same key,
 * so the server acts once; a success, or `forget` after a refusal, starts afresh.
 */
export function useKeyedWrite() {
  const [busy, setBusy] = useState(false);
  const attempt = useRef<Attempt | null>(null);

  const run = async (body: unknown, write: (key: string) => Promise<{ error?: Failure }>): Promise<WriteOutcome> => {
    const a = attemptFor(attempt.current, body);
    attempt.current = a;
    setBusy(true);
    try {
      const { error } = await write(a.key);
      if (error !== undefined) return { ok: false, failure: error };
      attempt.current = null;
      return { ok: true };
    } finally {
      setBusy(false);
    }
  };
  const forget = () => {
    attempt.current = null;
  };
  return { busy, run, forget };
}
