import { BookingBadge, type Lead, wishText } from "@/entities/lead";
import { BookingActions } from "@/features/manage-booking";
import { useLocale, useT } from "@/shared/i18n";
import { formatSlot } from "@/shared/lib/instant";

import { CardSection, FactList } from "./card-section";

/** The lead's booking: where it stands, through whom, when, how it was joined to the lead; then what the operator may do. */
export function BookingBlock({ lead, onChanged }: { lead: Lead; onChanged: () => void }) {
  const t = useT();
  const locale = useLocale();
  const b = lead.booking;
  const wish = wishText(b, t, locale);
  const rows = [
    b.provider && { label: t("booking.card.provider"), value: t(`booking.provider.${b.provider}`) },
    b.start_at && { label: t("booking.card.when"), value: formatSlot(b.start_at, b.end_at, locale) },
    wish && { label: t("booking.card.asked"), value: wish },
    b.match && { label: t("booking.card.match"), value: <span className={b.match === "contact" ? "text-accent-warn" : undefined}>{t(`booking.match.${b.match}`)}</span> },
    b.external_ref && { label: t("booking.card.ref"), value: <span className="font-mono text-xs">{b.external_ref}</span> },
  ].filter((r) => !!r);

  return (
    <CardSection title={t("card.booking")}>
      {b.status === "none" ? (
        <p className="text-sm text-ink-soft">{t("booking.card.none")}</p>
      ) : (
        <div className="flex flex-col gap-2">
          <div>
            <BookingBadge booking={b} />
          </div>
          {rows.length > 0 && <FactList rows={rows} />}
        </div>
      )}
      <BookingActions lead={lead} onChanged={onChanged} />
    </CardSection>
  );
}
