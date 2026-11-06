import { BOOKING_LIMITS as LIMIT, BOOKING_PROVIDERS, DAY_PARTS, PAGE_PROVIDERS, type DayPart } from "./generated/booking.js";
import type { BookingProvider, PageProvider } from "./generated/types.js";
import type { BookingRequestedV1 } from "./generated/wire.js";
import { isDate, isObject } from "./text.js";

export const isBookingProvider = (v: unknown): v is BookingProvider => BOOKING_PROVIDERS.some(p => p === v);
export const isPageProvider = (v: unknown): v is PageProvider => PAGE_PROVIDERS.some(p => p === v);

const LEAD_REF = /* @__PURE__ */ new RegExp(LIMIT.leadRef);

/** `lead-<row>-<8 lowercase hex>`, the row 1 or more: the panel's lead id for a site's lead. */
export const isLeadRef = (v: unknown): v is string => typeof v === "string" && LEAD_REF.test(v);

/** `booking.requested@1`'s properties, their vocabularies as the panel checks them. */
export type BookingRequested = BookingRequestedV1 & { provider: BookingProvider; preferred_part?: DayPart };

const isDayPart = (v: unknown): v is DayPart => DAY_PARTS.some(p => p === v);

/**
 * `booking.requested@1`'s properties, or `null` when the panel would refuse them: a lead ref, a
 * provider and, with `manual` only, a day and a part of the day. No null, nothing else, no free
 * text.
 */
export function bookingRequested(v: unknown): BookingRequested | null {
  if (!isObject(v) || Object.entries(v).some(([k, x]) => x === null || !["lead_ref", "provider", "preferred_date", "preferred_part"].includes(k))) return null;
  const { lead_ref, provider, preferred_date, preferred_part } = v;
  if (!isLeadRef(lead_ref) || !isBookingProvider(provider)) return null;
  if ((preferred_date !== undefined && !isDate(preferred_date)) || (preferred_part !== undefined && !isDayPart(preferred_part))) return null;
  if (provider !== "manual" && (preferred_date !== undefined || preferred_part !== undefined)) return null;
  return {
    lead_ref,
    provider,
    ...(preferred_date !== undefined ? { preferred_date } : {}),
    ...(preferred_part !== undefined ? { preferred_part } : {}),
  };
}

const DAY_MS = 86_400_000;

/**
 * Whether a visitor's preferred day may be asked for on `today` (both `YYYY-MM-DD`): two days
 * back (clocks, an outbox's delay) to a year and a day ahead. The panel checks it on arrival.
 */
export function preferredDateInWindow(day: string, today: string): boolean {
  const days = (Date.parse(`${day}T00:00:00Z`) - Date.parse(`${today}T00:00:00Z`)) / DAY_MS;
  return days >= -2 && days <= 366;
}
