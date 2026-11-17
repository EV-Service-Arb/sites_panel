import { MESSENGERS, type Messenger } from "@evinvest/sites-contract";

import type { Lead, operations } from "@/shared/api/schema";

import { CONTACT_SLA_SECONDS } from "./generated";

/**
 * The wait as of `now`, from `waiting_since` — the API's `waiting_seconds` and
 * `overdue` are as of the answer, and a list left open must keep counting.
 */
export function slaAt(sla: NonNullable<Lead["sla"]>, now: number): { seconds: number; overdue: boolean } {
  const since = Date.parse(sla.waiting_since);
  const seconds = Number.isNaN(since) ? sla.waiting_seconds : Math.max(sla.waiting_seconds, Math.floor((now - since) / 1000));
  return { seconds, overdue: sla.overdue || seconds > CONTACT_SLA_SECONDS };
}

/** The messenger a lead waits on: one it came through, the customer not having written yet. */
export function awaitingMessage(lead: Pick<Lead, "channel" | "messaged_at">): Messenger | null {
  if (lead.messaged_at !== null) return null;
  return MESSENGERS.find((m) => m === lead.channel) ?? null;
}

/** What narrows the queue, as `GET /leads` takes it; the GMBs are the host's selection, beside it. */
export type LeadFilter = Omit<NonNullable<operations["list_leads"]["parameters"]["query"]>, "gmb" | "cursor" | "limit">;

/** A lead's address in the API and in the page URL: `brand/lead`. */
export interface LeadRef {
  brand: string;
  lead: string;
}

export function refOf(lead: Pick<Lead, "brand" | "lead_id">): LeadRef {
  return { brand: lead.brand, lead: lead.lead_id };
}

export function encodeRef(ref: LeadRef): string {
  return `${ref.brand}/${ref.lead}`;
}

export function decodeRef(raw: string | null): LeadRef | null {
  if (!raw) return null;
  const at = raw.indexOf("/");
  if (at <= 0 || at === raw.length - 1) return null;
  return { brand: raw.slice(0, at), lead: raw.slice(at + 1) };
}

/** What the customer left, the known fields picked out as text. */
export interface Contact {
  name: string | null;
  phone: string | null;
  email: string | null;
  need: string | null;
}

/** What a customer left, or an event's properties: free JSON, an object when there is any, absent when not. */
export function fieldsOf(json: unknown): Readonly<Record<string, unknown>> {
  if (json === undefined) return {};
  if (typeof json !== "object" || json === null || Array.isArray(json)) throw new Error(`${JSON.stringify(json)} is not an object`);
  return json as Record<string, unknown>;
}

export function contactOf(pii: unknown): Contact {
  const fields = fieldsOf(pii);
  const text = (k: string) => {
    const v = fields[k];
    return typeof v === "string" && v.trim() !== "" ? v : null;
  };
  return { name: text("name"), phone: text("phone"), email: text("email"), need: text("need") };
}

/** Keys a site sends that have a label of their own; `need` may be a site's raw id (`hot_water`) and shows as sent. */
export const LABELLED_PII = ["locality", "bedrooms"] as const;
export type LabelledPii = (typeof LABELLED_PII)[number];

const CONTACT_KEYS: readonly string[] = ["name", "phone", "email", "need"];

/** A value the customer left, as plain text: never markup — it is their input. */
function asText(v: unknown): string | null {
  if (v === null || v === undefined) return null;
  if (typeof v === "string") return v.trim() === "" ? null : v;
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  return JSON.stringify(v);
}

/**
 * Everything else the customer left: the labelled keys first, in their order,
 * then the rest by key — a site may send fields the panel has no name for yet.
 */
export function extrasOf(pii: unknown): { labelled: { key: LabelledPii; value: string }[]; other: { key: string; value: string }[] } {
  const fields = fieldsOf(pii);
  const labelled = LABELLED_PII.flatMap((key) => {
    const value = asText(fields[key]);
    return value === null ? [] : [{ key, value }];
  });
  const known: readonly string[] = [...CONTACT_KEYS, ...LABELLED_PII];
  const other = Object.entries(fields)
    .filter(([k]) => !known.includes(k))
    .flatMap(([key, v]) => {
      const value = asText(v);
      return value === null ? [] : [{ key, value }];
    })
    .sort((a, b) => a.key.localeCompare(b.key));
  return { labelled, other };
}

/**
 * The number as a `tel:` link may carry it: `+` and digits only, whatever the
 * customer typed around them. Fewer than six digits is not a number to dial —
 * the page shows the text and offers no call.
 */
export function dialable(phone: string | null): string | null {
  if (!phone) return null;
  const tel = phone.replace(/[^\d+]/g, "");
  return tel.replace(/\D/g, "").length >= 6 ? tel : null;
}

/**
 * Whether a lead reached a stage, by the stage times the projection keeps (a
 * lead lost after a quote did reach "quoted"). The same rule the backend's
 * funnel counts by.
 */
export function reached(lead: Lead, stage: Exclude<Lead["stage"], "lost">): boolean {
  switch (stage) {
    case "created":
      return true;
    case "contacted":
      return lead.contacted_at !== null || reached(lead, "quoted");
    case "quoted":
      return lead.quoted_at !== null || reached(lead, "won");
    case "won":
      return lead.won_at !== null || reached(lead, "completed");
    case "completed":
      return lead.completed_at !== null || reached(lead, "paid");
    case "paid":
      return lead.paid_at !== null;
  }
}
