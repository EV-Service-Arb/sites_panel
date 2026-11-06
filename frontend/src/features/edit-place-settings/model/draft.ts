import { MESSENGERS, type Messenger, type PlaceProblem, checkPlaceLive } from "@evinvest/sites-contract";

import { type EditedFields, type PlaceSettings, messengerOn } from "@/entities/place";

import { type BookingDraft, bookingDraftOf, bookingOf } from "./booking-draft";
import { type HoursDraftRow, hoursDraftOf, hoursOf, hoursValid } from "./hours-draft";

/** The form's state: text as typed. An empty field means "the site's own". */
export interface SettingsDraft {
  phone: string;
  whatsapp: string;
  /** The bot's username as typed; a leading `@` is dropped on save. */
  telegram: string;
  /** Every switch spelled out: on unless the place turned it off. */
  messengers: Record<Messenger, boolean>;
  hours: HoursDraftRow[];
  serviceArea: string[];
  booking: BookingDraft;
}

export function draftOf(edited: EditedFields): SettingsDraft {
  return {
    phone: edited.phone ?? "",
    whatsapp: edited.whatsapp ?? "",
    telegram: edited.telegram ?? "",
    messengers: { whatsapp: messengerOn(edited, "whatsapp"), telegram: messengerOn(edited, "telegram") },
    hours: hoursDraftOf(edited.hours),
    serviceArea: [...(edited.serviceArea ?? [])],
    booking: bookingDraftOf(edited.booking),
  };
}

/** "+33 6 12-34.56 (78)" → "+33612345678": what people paste, as E.164 has it. */
export function normalisePhone(raw: string): string {
  return raw.replace(/[\s().-]/g, "");
}

/** "@aquafix_devis_bot " → "aquafix_devis_bot": the username as the server stores it. */
export function normaliseBot(raw: string): string {
  return raw.trim().replace(/^@/, "");
}

/** Only the switches turned off are stored: absent is on, so an all-on place keeps no `messengers` at all. */
function messengersOf(draft: Record<Messenger, boolean>): EditedFields["messengers"] | null {
  const off: EditedFields["messengers"] = {};
  for (const m of MESSENGERS) if (!draft[m]) off[m] = false;
  return Object.keys(off).length > 0 ? off : null;
}

/** A commune joins the list once, compared without case or surrounding space. */
export function addArea(list: readonly string[], raw: string): string[] {
  const name = raw.trim().replace(/\s+/g, " ");
  if (!name || list.some((n) => n.toLocaleLowerCase() === name.toLocaleLowerCase())) return [...list];
  return [...list, name];
}

export function removeArea(list: readonly string[], name: string): string[] {
  return list.filter((n) => n !== name);
}

export function editedOf(draft: SettingsDraft): EditedFields {
  const out: EditedFields = {};
  const phone = normalisePhone(draft.phone);
  const whatsapp = normalisePhone(draft.whatsapp);
  const telegram = normaliseBot(draft.telegram);
  const messengers = messengersOf(draft.messengers);
  const hours = hoursOf(draft.hours);
  if (phone) out.phone = phone;
  if (whatsapp) out.whatsapp = whatsapp;
  if (telegram) out.telegram = telegram;
  if (messengers) out.messengers = messengers;
  if (hours) out.hours = hours;
  if (draft.serviceArea.length > 0) out.serviceArea = [...draft.serviceArea];
  const booking = bookingOf(draft.booking);
  if (booking) out.booking = booking;
  return out;
}

/** The settings to PUT: the draft's fields over everything the form does not edit, as it came. */
export function settingsOf(draft: SettingsDraft, base: PlaceSettings): PlaceSettings {
  return { edited: editedOf(draft), rest: base.rest };
}

export type ProblemCode = PlaceProblem["code"] | "default_choose";

/** Why the server would refuse the draft, keyed as its 422 keys them; and the form's own: booking pages typed with no default picked. */
export function draftProblems(draft: SettingsDraft): Map<string, ProblemCode> {
  const problems = new Map<string, ProblemCode>(checkPlaceLive(editedOf(draft)).problems.map((p) => [p.path, p.code]));
  if (draft.booking.default === null && bookingOf(draft.booking) !== null) problems.set("booking.default", "default_choose");
  return problems;
}

export function draftValid(draft: SettingsDraft): boolean {
  return draftProblems(draft).size === 0 && hoursValid(draft.hours);
}

export function draftChanged(draft: SettingsDraft, base: PlaceSettings): boolean {
  return JSON.stringify(editedOf(draft)) !== JSON.stringify(normalisedEdited(base.edited));
}

/** The stored fields in the form's own key order, so an untouched form compares equal. */
function normalisedEdited(e: EditedFields): EditedFields {
  return editedOf(draftOf(e));
}
