import { IDEMPOTENCY_HEADER, http, ignoreBody } from "@/shared/api";

import { type UnmatchedBooking, unmatchedListParser } from "../model/booking";

/** The next slot first; one brand, or every brand for null. */
export async function fetchUnmatched(brand: string | null, limit = 100): Promise<UnmatchedBooking[]> {
  return (await http.get("/bookings/unmatched", unmatchedListParser, { brand, limit })).bookings;
}

/**
 * Joins the booking to a lead of its brand. 404: no such booking, or no such
 * lead of its brand; 409: an operator attached it there already.
 */
export async function attachBooking(id: string, lead: string, idempotencyKey: string): Promise<void> {
  await http.send("POST", `/bookings/${encodeURIComponent(id)}/attach`, { lead }, ignoreBody, { [IDEMPOTENCY_HEADER]: idempotencyKey });
}
