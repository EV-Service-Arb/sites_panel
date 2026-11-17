import type { Failure } from "@/shared/api";
import type { T } from "@/shared/i18n";

/** What to tell a person about a failed call: our words where theirs would mean nothing to them, else the service's. */
export function failureText(failure: Failure, t: T): string {
  switch (failure.code) {
    case "network":
      return t("state.network");
    case "csrf":
      return t("state.csrf");
    case "unauthenticated":
    case "forbidden":
      return t("state.forbidden");
    case "unavailable":
    case "bad_gateway":
    case "invoices_unavailable":
    case "ledger_unavailable":
    case "google_unavailable":
    case "not_granted":
      return t("state.unavailable.body");
    case "internal":
      return t("state.error", { detail: failure.code });
    case "already_attached":
    case "bad_request":
    case "booking_done":
    case "consent_expired":
    case "gmb_taken":
    case "google_refused":
    case "idempotency_conflict":
    case "idempotency_key_taken":
    case "invalid_pricing":
    case "invalid_settings":
    case "invoice_not_open":
    case "invoice_processing":
    case "ledger_refused":
    case "no_booking":
    case "no_won_job":
    case "not_booked":
    case "not_found":
    case "place_exists":
    case "scope_frozen":
    case "stale":
    case "too_large":
      return failure.error;
    default:
      return failure satisfies never;
  }
}
