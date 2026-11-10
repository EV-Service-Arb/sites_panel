import { Badge } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";

import { stateTone } from "../lib/state";
import type { InvoiceState } from "../model/invoice";

/** The row's state, as the panel stored it. */
export function InvoiceStateBadge({ state }: { state: InvoiceState }) {
  const t = useT();
  const label = t(`invoices.state.${state}`);
  switch (stateTone(state)) {
    case "positive":
      return <Badge tone="positive">{label}</Badge>;
    case "pending":
      return <Badge tone="neutral">{label}</Badge>;
    case "neutral":
      return <Badge tone="neutral">{label}</Badge>;
    case "ended":
      return (
        <Badge tone="neutral" className="text-ink-soft">
          {label}
        </Badge>
      );
  }
}
