import { useState } from "react";

import { type PlaceKey, type PlaceSettings, type PlaceSettingsView, savePlaceSettings } from "@/entities/place";
import { ApiError } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

import { type FieldErrors, NO_ERRORS, fieldErrorsOf } from "./field-errors";

export type SaveState = { kind: "idle" } | { kind: "saving" } | { kind: "conflict" } | { kind: "invalid"; errors: FieldErrors };

/**
 * A PUT against the `updated_at` the form was read at. A 409 and a 422 become
 * states the form shows in place; anything else is a toast.
 */
export function useSave(key: PlaceKey, expectedUpdatedAt: string | null, onSaved: (view: PlaceSettingsView) => void) {
  const t = useT();
  const [state, setState] = useState<SaveState>({ kind: "idle" });

  const save = async (settings: PlaceSettings) => {
    setState({ kind: "saving" });
    try {
      const view = await savePlaceSettings(key, settings, expectedUpdatedAt);
      setState({ kind: "idle" });
      onSaved(view);
    } catch (e) {
      if (e instanceof ApiError && e.failure.kind === "conflict") return setState({ kind: "conflict" });
      if (e instanceof ApiError && e.failure.kind === "invalid_fields") return setState({ kind: "invalid", errors: fieldErrorsOf(e.failure.fields) });
      setState({ kind: "idle" });
      notifyFailure(e, t);
    }
  };

  const errors = state.kind === "invalid" ? state.errors : NO_ERRORS;
  return { state, errors, save };
}
