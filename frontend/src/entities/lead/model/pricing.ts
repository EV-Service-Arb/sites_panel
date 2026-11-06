import type { LeadFlow } from "@evinvest/sites-contract";

/** A price the site committed to: only `estimate` and `fixed` carry one. */
export function quotedPrice(lead: { flow: LeadFlow | null; quoted_cents: number | null }): number | null {
  return lead.flow === "estimate" || lead.flow === "fixed" ? lead.quoted_cents : null;
}
