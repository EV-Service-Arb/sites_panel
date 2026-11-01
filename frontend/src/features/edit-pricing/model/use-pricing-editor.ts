import { toast } from "@evinvest/uikit";
import { useEffect, useMemo, useState } from "react";

import type { PricingItem, PricingModel } from "@/entities/pricing";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

import { focusField } from "../lib/focus";
import { type PricingDraft, draftOf, fingerprint } from "./draft";
import { type FieldErrors, type ServerProblem, errorsOf } from "./errors";
import { fieldOfPath } from "./paths";
import { type SaveOutcome, trySave } from "./save";
import { type Serialized, modelOf } from "./serialize";

export type EditorState = { kind: "idle" } | { kind: "saving" } | { kind: "conflict"; current: PricingItem | null };

export interface PricingEditor {
  base: PricingItem;
  draft: PricingDraft;
  update: (change: (d: PricingDraft) => PricingDraft) => void;
  serialized: Serialized;
  errors: FieldErrors;
  changed: boolean;
  state: EditorState;
  save: () => void;
  /** After a 409: the same draft again, over the pricing as it now is. */
  overwrite: () => void;
  /** Starts the draft again from a model (a pasted JSON); the draft stays unsaved. */
  replace: (model: PricingModel) => void;
  reset: () => void;
  /** Every problem shown, and the field a model path names in focus. */
  showPath: (path: string) => void;
  showFirst: () => void;
}

/**
 * The draft of one brand's pricing, from `base` (the pricing as read when the
 * draft started — re-mount on a new `base`). A failed save never touches the
 * draft: a 422 is filed under its field, a 409 waits for the person to load
 * the fresh pricing or write over it.
 */
export function usePricingEditor(base: PricingItem, today: string, onSaved: (item: PricingItem) => void): PricingEditor {
  const t = useT();
  const [draft, setDraft] = useState<PricingDraft>(() => draftOf(base.model, today));
  const [revealAll, setRevealAll] = useState(false);
  const [server, setServer] = useState<ServerProblem | null>(null);
  const [state, setState] = useState<EditorState>({ kind: "idle" });
  // A request, numbered, so asking for the same field twice focuses it twice.
  const [focusReq, setFocusReq] = useState<{ field: string | null; n: number }>({ field: null, n: 0 });
  const setFocusTo = (field: string | null) => setFocusReq((r) => ({ field, n: r.n + 1 }));
  const serialized = useMemo(() => modelOf(draft, base.locales), [draft, base.locales]);
  const pristine = useMemo(() => fingerprint(draftOf(base.model, today)), [base.model, today]);
  const errors = errorsOf(serialized.problems, revealAll, server);

  // Focus after the render that shows the field's errors, so the reader lands on them.
  useEffect(() => {
    if (focusReq.field !== null) focusField(focusReq.field);
  }, [focusReq]);

  const settle = (outcome: SaveOutcome) => {
    switch (outcome.kind) {
      case "saved":
        setState({ kind: "idle" });
        toast.positive(t("pricing.saved"));
        return onSaved(outcome.item);
      case "conflict":
        return setState({ kind: "conflict", current: outcome.current });
      case "invalid": {
        const field = fieldOfPath(outcome.path, draft);
        setServer({ field, path: outcome.path, message: outcome.message });
        setState({ kind: "idle" });
        return setFocusTo(field);
      }
      case "failed":
        setState({ kind: "idle" });
        return notifyFailure(outcome.error, t);
    }
  };
  const put = (expected: string | null) => {
    if (!serialized.model || serialized.problems.length > 0) {
      setRevealAll(true);
      setFocusTo(errorsOf(serialized.problems, true, null).first);
      return;
    }
    setState({ kind: "saving" });
    void trySave(base.brand_id, serialized.model, expected).then(settle);
  };

  return {
    base,
    draft,
    update: (change) => {
      setServer(null);
      setDraft(change);
    },
    serialized,
    errors,
    changed: fingerprint(draft) !== pristine,
    state,
    save: () => put(base.updated_at),
    overwrite: () => {
      if (state.kind === "conflict" && state.current) put(state.current.updated_at);
    },
    replace: (model) => {
      setServer(null);
      setDraft(draftOf(model, today));
    },
    reset: () => {
      setServer(null);
      setRevealAll(false);
      setDraft(draftOf(base.model, today));
    },
    showPath: (path) => {
      setRevealAll(true);
      setFocusTo(fieldOfPath(path, draft));
    },
    showFirst: () => {
      setRevealAll(true);
      setFocusTo(errorsOf(serialized.problems, true, null).first);
    },
  };
}
