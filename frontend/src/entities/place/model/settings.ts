import { type PlaceLive as ContractLive, checkBookingConfig } from "@evinvest/sites-contract";

import type { PlaceLive } from "@/shared/api/schema";

/**
 * The fields the panel edits in v1. Everything else a place's live data holds
 * (address, geo, storefrontPhoto, landmark, rating) stays in `rest`, untouched,
 * and goes back as it came: a PUT is a full replace.
 */
export const EDITED_KEYS = ["phone", "whatsapp", "telegram", "messengers", "hours", "serviceArea", "booking"] as const satisfies readonly (keyof ContractLive)[];
type EditedKey = (typeof EDITED_KEYS)[number];
export type EditedFields = Pick<ContractLive, EditedKey>;

export interface PlaceSettings {
  edited: EditedFields;
  rest: Omit<PlaceLive, EditedKey>;
}

const edits = (k: string, v: unknown): boolean =>
  // A booking the server would now refuse (its rules grew) is not the form's to edit: it rides in `rest`.
  (EDITED_KEYS as readonly string[]).includes(k) && (k !== "booking" || checkBookingConfig(v).config !== null);

/** The live data as stored, split into what the form edits and what it carries through. */
export function splitLive(live: PlaceLive): PlaceSettings {
  const entries = Object.entries(live);
  return {
    edited: Object.fromEntries(entries.filter(([k, v]) => edits(k, v))) as EditedFields,
    rest: Object.fromEntries(entries.filter(([k, v]) => !edits(k, v))) as Omit<PlaceLive, EditedKey>,
  };
}

/** The wire object of a PUT: what the form says over what it does not edit. */
export function settingsToWire(s: PlaceSettings): PlaceLive {
  return { ...s.rest, ...s.edited };
}
