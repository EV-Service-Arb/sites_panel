import { type Lead, PriceText, quotedPrice } from "@/entities/lead";
import { useLocale, useT } from "@/shared/i18n";
import { formatDay } from "@/shared/lib/format";

import { CardSection, FactList } from "./card-section";

/**
 * How the site priced the need: the variant, the price it committed to and the
 * day of the price model behind it, and the visitor's choices. The choices are
 * the model's ids as sent — their labels arrive with the pricing editor.
 */
export function PricingBlock({ lead }: { lead: Lead }) {
  const t = useT();
  const locale = useLocale();
  if (lead.flow === null) return null;
  const price = quotedPrice(lead);
  const rows = [
    { label: t("card.pricing.flow"), value: t(`flow.${lead.flow}`) },
    { label: t("card.pricing.price"), value: price === null ? t("card.pricing.onQuote") : <PriceText cents={price} /> },
    ...(lead.pricing_valid_from === null ? [] : [{ label: t("card.pricing.validFrom"), value: formatDay(lead.pricing_valid_from, locale) }]),
  ];
  const inputs = Object.entries(lead.estimate_inputs ?? {});
  return (
    <CardSection title={t("card.pricing")}>
      <FactList rows={rows} />
      {inputs.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-sm text-ink-soft">{t("card.pricing.inputs")}</span>
          <FactList rows={inputs.map(([key, value]) => ({ label: key, value: <span className="font-mono text-xs">{value}</span>, mono: true }))} />
        </div>
      )}
    </CardSection>
  );
}
