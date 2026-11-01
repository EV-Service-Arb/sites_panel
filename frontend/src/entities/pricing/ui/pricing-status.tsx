import { Badge, Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@evinvest/uikit";
import { type ReactNode, useEffect, useId, useRef } from "react";

import { useLocale, useT } from "@/shared/i18n";
import { formatDay } from "@/shared/lib/format";

import { pricingStamp } from "../lib/stamp";
import type { PricingItem } from "../model/item";

export interface PricingStatusProps {
  item: PricingItem;
  action?: ReactNode;
  /** Take focus when shown: the screen was rebuilt under the person (fresh pricing loaded), so focus must land somewhere named. */
  focusOnMount?: boolean;
}

/** What the brand's sites price from now: the panel's model (since when, saved by whom), or their baked one. */
export function PricingStatus({ item, action, focusOnMount = false }: PricingStatusProps) {
  const t = useT();
  const locale = useLocale();
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const stamp = pricingStamp(item, locale);
  // Read once: a later render of the same card must not pull focus back.
  const focusFirst = useRef(focusOnMount);
  useEffect(() => {
    if (focusFirst.current) ref.current?.focus();
  }, []);
  return (
    <Card ref={ref} role="region" aria-labelledby={titleId} tabIndex={-1} className="outline-none focus-visible:ring-3 focus-visible:ring-ring/50">
      <CardHeader>
        <CardTitle className="flex flex-wrap items-center gap-2">
          <span id={titleId}>{t("pricing.status.title")}</span>
          {item.model ? <Badge variant="success">{t("pricing.status.live")}</Badge> : <Badge variant="outline">{t("pricing.status.baked")}</Badge>}
        </CardTitle>
        <CardDescription>{item.model ? t("pricing.status.validFrom", { day: formatDay(item.model.validFrom, locale) }) : t("pricing.status.none")}</CardDescription>
        {action && <CardAction>{action}</CardAction>}
      </CardHeader>
      <CardContent className="flex flex-col gap-1 text-sm text-ink-soft">
        {stamp && <span>{t(stamp.what === "saved" ? "pricing.status.saved" : "pricing.status.cleared", { at: stamp.at, by: stamp.by })}</span>}
        <span>{t("pricing.status.locales", { locales: item.locales.join(", ") || "—" })}</span>
      </CardContent>
    </Card>
  );
}
