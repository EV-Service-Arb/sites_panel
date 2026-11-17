import type { BookingContact } from "@/entities/booking";
import { type Lead, contactOf } from "@/entities/lead";

const digits = (s: string | null | undefined) => (s ?? "").replace(/\D/g, "");

/**
 * Whether a lead left the contact the attendee gave the provider: the same
 * email (any case), or the same number however written ("06…" against
 * "+33 6…": the last nine digits, a French number without its prefix).
 */
/** `contact` absent: a caller who sees no PII. */
export function sameContact(lead: Lead, contact: BookingContact | null | undefined): boolean {
  if (contact === null || contact === undefined) return false;
  const c = contactOf(lead.pii);
  const email = contact.email?.trim().toLowerCase();
  if (email && c.email?.trim().toLowerCase() === email) return true;
  const a = digits(contact.phone);
  const b = digits(c.phone);
  return a.length >= 9 && b.length >= 9 && a.slice(-9) === b.slice(-9);
}

/** What the search box matches a lead by: its words and its id, lowercase as the kit's Command compares. */
export function candidateText(lead: Lead): string {
  const c = contactOf(lead.pii);
  return [c.need, c.name, c.phone, c.email, lead.location, lead.lead_id].filter(Boolean).join(" ").toLowerCase();
}

/** The leads to offer: those with the booking's contact first, then the newest, as listed. */
export function rankCandidates(leads: readonly Lead[], contact: BookingContact | null | undefined): { lead: Lead; same: boolean }[] {
  const rows = leads.map((lead) => ({ lead, same: sameContact(lead, contact) }));
  return [...rows.filter((r) => r.same), ...rows.filter((r) => !r.same)];
}

/** A typed id worth offering as is: one word, as lead ids are. */
export function typedId(search: string): string | null {
  const id = search.trim();
  return id !== "" && !/\s/.test(id) ? id : null;
}
