import type { Messenger } from "@evinvest/sites-contract";

import type { EditedFields } from "../model/settings";
import { type LocalTime, isOpenAt } from "./hours";

/** The ways a visitor reaches the place: a call, WhatsApp, the place's Telegram bot, or the form asking to be called back. */
export type Channel = "call" | "whatsapp" | "telegram" | "callback";

export interface ChannelSlot {
  channel: Channel;
  /** The panel's number or bot; null where the site keeps its own (unknown here) or, for the form, none applies. */
  value: string | null;
  /** Set in the panel, or left to the site's own config. */
  from: "panel" | "site";
}

export interface ChannelPlan {
  /** "unknown" when the hours are the site's own, which the panel does not see. */
  state: "open" | "closed" | "unknown";
  slots: ChannelSlot[];
}

const OPEN_ORDER: readonly Channel[] = ["call", "whatsapp", "telegram", "callback"];
const CLOSED_ORDER: readonly Channel[] = ["callback", "whatsapp", "telegram", "call"];

/** Whether the landing shows a messenger's button: on unless the panel switched it off. */
export function messengerOn(fields: Pick<EditedFields, "messengers">, messenger: Messenger): boolean {
  return fields.messengers?.[messenger] !== false;
}

/**
 * What the site will put in front of a visitor at `at`: open → the call first,
 * closed → the callback form and WhatsApp first, as nobody answers the phone.
 * Only what the panel knows is named; a channel the site supplies itself is
 * listed as the site's own, never filled in. A messenger switched off is left
 * out: the lead form stops offering it (other buttons on the site stay).
 * Telegram is listed only with a bot named here.
 */
export function channelPreview(fields: EditedFields, at: LocalTime): ChannelPlan {
  const state = fields.hours && fields.hours.length > 0 ? (isOpenAt(fields.hours, at) ? "open" : "closed") : "unknown";
  const shown = (channel: Channel): boolean => {
    if (channel === "whatsapp") return messengerOn(fields, "whatsapp");
    if (channel === "telegram") return messengerOn(fields, "telegram") && fields.telegram !== undefined;
    return true;
  };
  const slot = (channel: Channel): ChannelSlot => {
    if (channel === "callback") return { channel, value: null, from: "panel" };
    const value = channel === "call" ? fields.phone : channel === "whatsapp" ? fields.whatsapp : fields.telegram && `@${fields.telegram}`;
    return value ? { channel, value, from: "panel" } : { channel, value: null, from: "site" };
  };
  return { state, slots: (state === "closed" ? CLOSED_ORDER : OPEN_ORDER).filter(shown).map(slot) };
}
