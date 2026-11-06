import type { BookingStatus } from "@evinvest/sites-contract";
import { Badge, type BadgeVariant } from "@evinvest/uikit";

import { useLocale, useT } from "@/shared/i18n";

import { bookingWhen } from "../lib/booking-text";
import type { LeadBooking } from "../model/booking";

const VARIANT: Record<Exclude<BookingStatus, "none">, BadgeVariant> = {
  requested: "outline",
  booked: "success",
  canceled: "outline",
  done: "secondary",
  no_show: "destructive",
};

/** Where the booking stands; nothing at all for a lead without one. */
export function BookingBadge({ booking }: { booking: LeadBooking }) {
  const t = useT();
  if (booking.status === "none") return null;
  return <Badge variant={VARIANT[booking.status]}>{t(`booking.status.${booking.status}`)}</Badge>;
}

/**
 * The badge with its when beside it, for a list row. A booking joined to the
 * lead by its contact alone is a guess the operator should see as one.
 */
export function BookingLine({ booking }: { booking: LeadBooking }) {
  const t = useT();
  const locale = useLocale();
  if (booking.status === "none") return <span className="text-ink-soft">—</span>;
  const when = bookingWhen(booking, t, locale);
  return (
    <span className="flex flex-wrap items-center gap-x-1.5 gap-y-0.5">
      <BookingBadge booking={booking} />
      {when && <span className="text-xs tabular-nums text-ink-mid">{when}</span>}
      {booking.match === "contact" && (
        <span className="text-xs text-accent-warn" title={t("booking.match.contact.hint")}>
          {t("booking.match.contact.short")}
        </span>
      )}
    </span>
  );
}
