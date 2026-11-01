import { Button } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * Arrivals wait here instead of pushing the rows down under the pointer. From
 * `md` up the banner takes no room either: it floats over the table's header
 * (zero height, the column's gap given back), sticky while the list scrolls. On
 * a phone it stays in the flow — floating, it would cover the first row's title.
 * The live region is always mounted, so a screen reader hears the count appear.
 */
export function NewLeadsBanner({ count, suspect, onShow }: { count: number; suspect: number; onShow: () => void }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <div aria-live="polite" className="empty:hidden md:sticky md:top-2 md:z-10 md:-mb-4 md:flex md:h-0 md:justify-center">
      {count > 0 && (
        <div
          data-enter="rise"
          className="flex items-center justify-between gap-3 rounded-lg border border-primary-ink/40 bg-primary-ink/10 px-3 py-2 md:h-fit md:rounded-full md:bg-card md:py-1 md:pl-4 md:pr-1 md:shadow-lg"
        >
          <span className="text-sm font-medium text-ink">
            {t("leads.live.new", { n: count })}
            {/* Counted with the rest, as the nav counts them, but said: they may not be worth a call. */}
            {suspect > 0 && <span className="font-normal text-accent-warn"> {t("leads.live.suspect", { n: suspect })}</span>}
          </span>
          <Button variant="outline" size={button("sm")} className="md:rounded-full" onClick={onShow}>
            {t("leads.live.show")}
          </Button>
        </div>
      )}
    </div>
  );
}
