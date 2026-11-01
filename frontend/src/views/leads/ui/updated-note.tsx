import { RefreshCw } from "lucide-react";

import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";

/**
 * A thin line over the card: what is below changed while it was open. The live
 * event names no person, so the note says when and what, not who.
 */
export function UpdatedNote({ at, type }: { at: string; type: string | null }) {
  const t = useT();
  const locale = useLocale();
  return (
    <p role="status" data-enter="rise" className="flex items-center gap-2 rounded-md border border-primary-ink/40 bg-primary-ink/10 px-3 py-1.5 text-xs text-ink">
      <RefreshCw aria-hidden className="size-3.5 shrink-0 text-primary-ink" />
      <span>
        {t("card.updatedElsewhere", { at: formatDateTime(at, locale) })}
        {type !== null && <span className="font-mono text-ink-mid"> · {type}</span>}
      </span>
    </p>
  );
}
