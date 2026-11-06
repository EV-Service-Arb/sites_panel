/**
 * Form variants for the dev stub (FORM-VARIANTS-SPEC.md): the pricing fields
 * on some leads, and the `?flow=` filter the leads list takes.
 */

import { LEAD_FLOWS, type LeadFlow } from "@evinvest/sites-contract";

export interface StubDeal {
  flow: LeadFlow;
  quoted_cents: number | null;
  pricing_valid_from: string | null;
  estimate_inputs: Record<string, string> | null;
}

/** A lead as far as this module needs it. */
interface Priced {
  lead_id: string;
  deal: StubDeal | null;
}

const MODEL_DAY = "2026-09-28";

export const estimate = (cents: number, inputs: Record<string, string>): StubDeal => ({ flow: "estimate", quoted_cents: cents, pricing_valid_from: MODEL_DAY, estimate_inputs: inputs });
export const fixed = (cents: number): StubDeal => ({ flow: "fixed", quoted_cents: cents, pricing_valid_from: MODEL_DAY, estimate_inputs: null });
export const quote = (): StubDeal => ({ flow: "quote", quoted_cents: null, pricing_valid_from: null, estimate_inputs: null });

/** The variants on the stub's seeded leads, by lead id. */
export function seedDeals(leads: Priced[]): void {
  const deals: Record<string, StubDeal> = {
    "stub-1": quote(),
    "stub-2": fixed(8_900),
    "stub-3": fixed(14_950),
    "stub-7": estimate(12_350, { zone: "paris-intra", bedrooms: "2", surface: "50-80", frequency: "every_2_weeks" }),
    "stub-11": estimate(9_000, { zone: "paris-intra", bedrooms: "studio", surface: "lt-30", frequency: "once" }),
  };
  for (const l of leads) l.deal = deals[l.lead_id] ?? l.deal;
}

/** The lead's pricing fields as `/leads` gives them; all null on a lead without a variant. */
export function dealDto(deal: StubDeal | null): StubDeal | { [K in keyof StubDeal]: null } {
  return deal ?? { flow: null, quoted_cents: null, pricing_valid_from: null, estimate_inputs: null };
}

/** `?flow=`: null for none, false for a word the API refuses with 400. */
export function flowParam(raw: string | null): LeadFlow | null | false {
  if (raw === null) return null;
  return LEAD_FLOWS.find((f) => f === raw) ?? false;
}
