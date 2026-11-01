import { Alert, AlertDescription, AlertTitle, Button } from "@evinvest/uikit";

import { type PricingItem, pricingStamp } from "@/entities/pricing";
import { type T, useLocale, useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/** "Saved … by …" or, when the write took the pricing off, "Taken off … by …". */
function whoWhen(item: PricingItem, locale: string, t: T, alert: "conflict" | "fresher"): string {
  const stamp = pricingStamp(item, locale) ?? { what: item.model ? "saved" : "cleared", at: "—", by: item.updated_by ?? "—" };
  const vars = { at: stamp.at, by: stamp.by };
  if (alert === "conflict") return stamp.what === "saved" ? t("pricing.conflict.body", vars) : t("pricing.conflict.bodyCleared", vars);
  return stamp.what === "saved" ? t("pricing.fresher.body", vars) : t("pricing.fresher.bodyCleared", vars);
}

/**
 * A save refused with 409: someone saved first. The draft stays; the person
 * either drops it for the fresh pricing or writes it over the fresh pricing.
 */
export function ConflictAlert({ current, onTakeFresh, onOverwrite }: { current: PricingItem | null; onTakeFresh: () => void; onOverwrite: () => void }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  return (
    <Alert variant="destructive" className="flex flex-col gap-2" role="alert">
      <AlertTitle>{t("pricing.conflict.title")}</AlertTitle>
      <AlertDescription>{current ? whoWhen(current, locale, t, "conflict") : t("pricing.conflict.unknown")}</AlertDescription>
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" size={button("sm")} onClick={onTakeFresh}>
          {t("pricing.conflict.load")}
        </Button>
        {current && (
          <Button type="button" variant="destructive" size={button("sm")} onClick={onOverwrite}>
            {t("pricing.conflict.overwrite")}
          </Button>
        )}
      </div>
    </Alert>
  );
}

/** Someone saved while this draft holds edits: nothing is lost yet, and a save would be refused. */
export function FresherAlert({ fresher, onTakeFresh }: { fresher: PricingItem; onTakeFresh: () => void }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  return (
    <Alert className="flex flex-col gap-2 border-accent-warn" data-enter="rise">
      <AlertTitle>{t("pricing.fresher.title")}</AlertTitle>
      <AlertDescription>{whoWhen(fresher, locale, t, "fresher")}</AlertDescription>
      <Button type="button" variant="outline" size={button("sm")} className="self-start" onClick={onTakeFresh}>
        {t("pricing.conflict.load")}
      </Button>
    </Alert>
  );
}
