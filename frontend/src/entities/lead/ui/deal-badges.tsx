import { type LeadFlow, PRICING_CURRENCY } from "@evinvest/sites-contract";
import { Badge, cn } from "@evinvest/uikit";

import { useLocale, useT } from "@/shared/i18n";
import { formatCents } from "@/shared/lib/format";

import { quotedPrice } from "../model/pricing";

/** The form variant, short in a row, spelled out on the card. `quote` is today's default, so it stays quiet. */
export function FlowBadge({ flow, short = false }: { flow: LeadFlow | null; short?: boolean }) {
  const t = useT();
  if (flow === null) return null;
  return (
    <Badge variant="outline" title={t(`flow.${flow}`)} className={cn(flow === "quote" && "text-ink-soft")}>
      {short ? t(`flow.short.${flow}`) : t(`flow.${flow}`)}
    </Badge>
  );
}

/** A quoted price to the cent, in the reader's number format. */
export function PriceText({ cents, className }: { cents: number; className?: string }) {
  const locale = useLocale();
  return <span className={cn("tabular-nums", className)}>{formatCents(cents, PRICING_CURRENCY, locale)}</span>;
}

/** A row's variant and, when the site committed to one, its price; nothing for a lead without a variant. */
export function DealSummary({ lead }: { lead: { flow: LeadFlow | null; quoted_cents: number | null } }) {
  const price = quotedPrice(lead);
  if (lead.flow === null && price === null) return null;
  return (
    <span className="flex flex-wrap items-center gap-1.5 text-sm">
      <FlowBadge flow={lead.flow} short />
      {price !== null && <PriceText cents={price} className="text-ink" />}
    </span>
  );
}
