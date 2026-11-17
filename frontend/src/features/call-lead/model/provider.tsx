import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";

import type { LeadRef } from "@/entities/lead";
import { type Failure, api } from "@/shared/api";
import type { CallOutcome } from "@/shared/api/schema";

import { type CallApi, CallFlow, type CallState, documentVisibility } from "./call-flow";

interface CallFlowValue {
  state: CallState;
  start: (ref: LeadRef) => Promise<Failure | null>;
  ask: () => void;
  dismiss: () => void;
  answer: (outcome: CallOutcome) => Promise<Failure | null>;
}

const CallFlowContext = createContext<CallFlowValue | null>(null);

const API: CallApi = {
  async attempt(ref) {
    const { data, error } = await api().POST("/leads/{brand}/{lead}/calls/attempt", { params: { path: ref } });
    return error === undefined ? { attemptId: data.attempt_id } : { failure: error };
  },
  async outcome(ref, attempt, outcome) {
    const { error } = await api().POST("/leads/{brand}/{lead}/calls/{attempt}/outcome", { params: { path: { ...ref, attempt } }, body: { outcome } });
    return error ?? null;
  },
};

/** One call at a time for the whole Leads screen, so the question outlives the card. */
export function CallFlowProvider({ onLogged, children }: { onLogged: (ref: LeadRef) => void; children: ReactNode }) {
  const [state, setState] = useState<CallState>({ phase: "idle" });
  const flow = useRef<CallFlow | null>(null);

  useEffect(() => {
    const f = new CallFlow(API, documentVisibility(document), setState);
    flow.current = f;
    return () => f.dispose();
  }, []);

  const answer = useCallback(
    async (outcome: CallOutcome) => {
      const f = flow.current;
      if (!f || f.current.phase !== "asking") return null;
      const { ref } = f.current;
      const refused = await f.answer(outcome);
      if (refused === null) onLogged(ref);
      return refused;
    },
    [onLogged],
  );

  const value = useMemo<CallFlowValue>(
    () => ({
      state,
      start: (ref) => flow.current?.start(ref) ?? Promise.resolve(null),
      ask: () => flow.current?.ask(),
      dismiss: () => flow.current?.dismiss(),
      answer,
    }),
    [state, answer],
  );

  return <CallFlowContext.Provider value={value}>{children}</CallFlowContext.Provider>;
}

export function useCallFlow(): CallFlowValue {
  const v = useContext(CallFlowContext);
  if (!v) throw new Error("useCallFlow outside CallFlowProvider");
  return v;
}
