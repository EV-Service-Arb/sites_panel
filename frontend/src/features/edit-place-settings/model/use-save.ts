import { useState } from "react";

import { type PlaceKey, type PlaceSettings, type PlaceSettingsView, settingsToWire } from "@/entities/place";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

/** `invalid`: the server's own words, the first problem it found (the form ran the same check before sending). */
export type SaveState = { kind: "idle" } | { kind: "saving" } | { kind: "conflict" } | { kind: "invalid"; message: string };

/**
 * A PUT against the `updated_at` the form was read at. A stale one and a refused one become
 * states the form shows in place; anything else is a toast.
 */
export function useSave(key: PlaceKey, expectedUpdatedAt: string | null, onSaved: (view: PlaceSettingsView) => void) {
  const t = useT();
  const [state, setState] = useState<SaveState>({ kind: "idle" });

  const save = async (settings: PlaceSettings) => {
    setState({ kind: "saving" });
    const { data, error } = await api().PUT("/places/{brand}/{slug}/settings", {
      params: { path: key },
      body: { settings: settingsToWire(settings), expected_updated_at: expectedUpdatedAt },
    });
    if (error?.code === "stale") return setState({ kind: "conflict" });
    if (error?.code === "invalid_settings") return setState({ kind: "invalid", message: error.error });
    setState({ kind: "idle" });
    if (error !== undefined) return notifyFailure(error, t);
    onSaved(data);
  };

  return { state, save };
}
