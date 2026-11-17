import type { BookingStatus } from "@/shared/api/schema";

import { BOOKING_ACTIONS } from "./generated";

/** What an operator may do from where a booking stands: anything else answers 409. */
export type BookingAction = (typeof BOOKING_ACTIONS)[BookingStatus][number];

export function bookingActions(status: BookingStatus): readonly BookingAction[] {
  return BOOKING_ACTIONS[status];
}
