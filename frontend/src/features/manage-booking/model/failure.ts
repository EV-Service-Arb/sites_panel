import type { Failure } from "@/shared/api";
import type { MessageKey } from "@/shared/i18n";

/**
 * What a refused booking write means to the operator. A conflict is the booking
 * moved on meanwhile (an operator here, the provider there): the card is read
 * again and says how it stands. Null: the general failure text applies.
 */
export function bookingRefusal(failure: Failure): { key: MessageKey; detail?: string; reread: boolean } | null {
  switch (failure.code) {
    case "booking_done":
    case "no_booking":
    case "not_booked":
      return { key: "booking.conflict", reread: true };
    case "not_found":
      return { key: "booking.notFound", reread: true };
    case "bad_request":
      return { key: "booking.badTime", detail: failure.error, reread: false };
    default:
      return null;
  }
}
