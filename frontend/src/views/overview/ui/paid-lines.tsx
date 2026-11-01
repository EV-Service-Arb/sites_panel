import type { Paid } from "@/entities/funnel";
import { useLocale, useT } from "@/shared/i18n";

import { paidLines } from "../model/paid";

/** "€2,310 · commission €231", one line per currency, under the "Paid" step that already names it. */
export function PaidLines({ payments }: { payments: readonly Paid[] }) {
  const t = useT();
  const locale = useLocale();
  const lines = paidLines(payments, locale);
  if (lines.length === 0) return null;
  return (
    <ul className="col-span-full flex flex-col gap-0.5 text-sm">
      {lines.map((line) => (
        <li key={line.currency} className="flex flex-wrap gap-x-2 tabular-nums">
          <span className="font-medium text-ink">{line.billed}</span>
          {line.commission && <span className="text-ink-soft">{t("funnel.paid.commission", { amount: line.commission })}</span>}
        </li>
      ))}
    </ul>
  );
}
