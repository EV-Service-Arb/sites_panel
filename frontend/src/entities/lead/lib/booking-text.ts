import type { T } from "@/shared/i18n";
import { formatDay } from "@/shared/lib/format";
import { formatSlot } from "@/shared/lib/instant";

import type { LeadBooking } from "@/shared/api/schema";

/** The visitor's wish as words, "6 Oct 2026, morning"; null when they asked for no day nor part. */
export function wishText(b: LeadBooking, t: T, locale: string): string | null {
  const parts = [b.preferred_date && formatDay(b.preferred_date, locale), b.preferred_part && t(`booking.part.${b.preferred_part}`)];
  const wish = parts.filter(Boolean).join(", ");
  return wish === "" ? null : wish;
}

/**
 * When, in words: the slot in the reader's zone, named; or, for a slot only
 * asked for, the visitor's wish ("asked for 6 Oct 2026, morning"). Null when
 * there is nothing to say.
 */
export function bookingWhen(b: LeadBooking, t: T, locale: string, timeZone?: string): string | null {
  if (b.start_at !== null) return formatSlot(b.start_at, b.end_at, locale, timeZone);
  const wish = b.status === "requested" ? wishText(b, t, locale) : null;
  return wish === null ? null : t("booking.requestedFor", { wish });
}
