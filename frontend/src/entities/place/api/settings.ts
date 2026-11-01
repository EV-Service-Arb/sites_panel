import { http } from "@/shared/api";

import { type PlaceSettings, type PlaceSettingsView, type SettingsChange, historyParser, placeSettingsParser, settingsToWire } from "../model/settings";

/** A place as the settings routes name it: brand and slug, both path segments. */
export interface PlaceKey {
  brand: string;
  slug: string;
}

const placePath = ({ brand, slug }: PlaceKey) => `/places/${encodeURIComponent(brand)}/${encodeURIComponent(slug)}`;

export function fetchPlaceSettings(key: PlaceKey): Promise<PlaceSettingsView> {
  return http.get(`${placePath(key)}/settings`, placeSettingsParser);
}

/**
 * A full replace. `expectedUpdatedAt` is the `updated_at` the form was read at;
 * a newer one on the server answers 409 rather than overwriting someone's change.
 */
export function savePlaceSettings(key: PlaceKey, settings: PlaceSettings, expectedUpdatedAt: string | null): Promise<PlaceSettingsView> {
  return http.send("PUT", `${placePath(key)}/settings`, { settings: settingsToWire(settings), expected_updated_at: expectedUpdatedAt }, placeSettingsParser);
}

export async function fetchSettingsHistory(key: PlaceKey): Promise<SettingsChange[]> {
  return (await http.get(`${placePath(key)}/settings/history`, historyParser)).changes;
}

/**
 * The change's `before` becomes current; the revert is itself a change in the
 * history. Guarded like a save: a newer `updated_at` on the server answers 409.
 */
export function revertSettingsChange(key: PlaceKey, changeId: string, expectedUpdatedAt: string | null): Promise<PlaceSettingsView> {
  return http.send("POST", `${placePath(key)}/settings/revert/${encodeURIComponent(changeId)}`, { expected_updated_at: expectedUpdatedAt }, placeSettingsParser);
}

/** Admin only: the site answers 404 for a withdrawn place until it is restored. */
export function setPlaceWithdrawn(key: PlaceKey, withdrawn: boolean): Promise<PlaceSettingsView> {
  return http.send("POST", `${placePath(key)}/${withdrawn ? "withdraw" : "restore"}`, undefined, placeSettingsParser);
}

/** Admin only: a place no lead or visit has named yet. 409 `exists` when it is registered already. */
export function addPlace(key: PlaceKey): Promise<PlaceSettingsView> {
  return http.send("POST", "/places", key, placeSettingsParser);
}
