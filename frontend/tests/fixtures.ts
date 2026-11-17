import type { Lead, LeadBooking } from "@/shared/api/schema";

export const NO_BOOKING: LeadBooking = { status: "none", provider: null, start_at: null, end_at: null, external_ref: null, match: null, preferred_date: null, preferred_part: null };

/** A lead as `GET /leads` answers one: a new one, nothing reached past its creation, `fields` over it. */
export function lead(fields: Partial<Lead> = {}): Lead {
  return {
    brand: "aquafix",
    lead_id: "lead-42-0a1b2c3d",
    location: null,
    job_id: null,
    stage: "created",
    channel: null,
    message_ref: null,
    messaged_at: null,
    messaged_channel: null,
    manual: false,
    created_at: null,
    contacted_at: null,
    quoted_at: null,
    won_at: null,
    completed_at: null,
    paid_at: null,
    lost_at: null,
    lost_reason: null,
    suspect: null,
    last_event_at: "2026-10-04T08:00:00Z",
    sla: null,
    flow: null,
    quoted_cents: null,
    pricing_valid_from: null,
    estimate_inputs: null,
    booking: NO_BOOKING,
    ...fields,
  };
}
