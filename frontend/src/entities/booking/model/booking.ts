import { BOOKING_PROVIDERS } from "@evinvest/sites-contract";

import { type Infer, arrayOf, nullable, object, oneOf, str } from "@/shared/lib/parse";

/** What an attendee left at the provider, for the roles that see it; each part may be missing. */
const contactParser = object({ name: nullable(str), email: nullable(str), phone: nullable(str) });
export type BookingContact = Infer<typeof contactParser>;

/**
 * A provider's booking no lead was found for (`GET /bookings/unmatched`):
 * `lead_id` and `match` are null by definition, so they are not read.
 */
export const unmatchedBookingParser = object({
  id: str,
  brand: str,
  provider: oneOf(BOOKING_PROVIDERS),
  external_ref: str,
  status: oneOf(["booked", "canceled"] as const),
  start_at: str,
  end_at: nullable(str),
  booked_at: nullable(str),
  last_event_at: str,
  contact: nullable(contactParser),
});
export type UnmatchedBooking = Infer<typeof unmatchedBookingParser>;

export const unmatchedListParser = object({ bookings: arrayOf(unmatchedBookingParser) });
