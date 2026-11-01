import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";

import type { LeadRef } from "@/entities/lead";

import { attemptCall, logCallOutcome } from "../api/calls";
import { CallFlow, type CallOutcome, type CallState, documentVisibility } from "./call-flow";

interface CallFlowValue {
  state: CallState;
  start: (ref: LeadRef) => Promise<void>;
  ask: () => void;
  dismiss: () => void;
  answer: (outcome: CallOutcome) => Promise<void>;
}

const CallFlowContext = createContext<CallFlowValue | null>(null);

const API = { attempt: attemptCall, outcome: logCallOutcome };

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
      if (!f || f.current.phase !== "asking") return;
      const { ref } = f.current;
      await f.answer(outcome);
      onLogged(ref);
    },
    [onLogged],
  );

  const value = useMemo<CallFlowValue>(
    () => ({
      state,
      start: (ref) => flow.current?.start(ref) ?? Promise.resolve(),
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
