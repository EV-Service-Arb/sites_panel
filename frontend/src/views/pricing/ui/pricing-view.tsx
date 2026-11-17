import { useResource } from "@evinvest/api/react";
import { Settled, Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import { useSelectedBrands } from "@/entities/place";
import type { PricingItem } from "@/entities/pricing";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { utcDay } from "@/shared/lib/format";
import { openView, useViewParams } from "@/shared/lib/router";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { ScreenFrame } from "@/shared/ui/screen-frame";

import { BrandPricing } from "./brand-pricing";
import { BrandSelect } from "./brand-select";

/**
 * A brand's prices as its sites quote them: the model in the panel (or none:
 * the sites' baked one), its editor for an admin, a preview priced by the
 * server, the JSON way in and out, and the history. The brands the host's
 * selection has a place of.
 */
export function PricingView() {
  const t = useT();
  // Fixed for the page's life: a new empty draft's "valid from" must not move under it at midnight.
  const [today] = useState(() => utcDay(new Date()));
  const all = useResource(api(), "/pricing");
  const brands = useSelectedBrands();
  const failure = all.status === "error" ? all.failure : brands.status === "error" ? brands.failure : undefined;
  const selected = brands.status === "ok" ? brands.data : undefined;
  const ready = all.status === "ok" && selected !== undefined;
  const items: PricingItem[] = !ready ? [] : selected === null ? all.data.items : all.data.items.filter((i) => selected.has(i.brand_id));
  const wanted = useViewParams("pricing").brand;
  const brand = items.find((i) => i.brand_id === wanted)?.brand_id ?? items[0]?.brand_id ?? null;
  const choose = (b: string) => openView({ view: "pricing", params: { brand: b } }, { replace: true });

  return (
    <ScreenFrame title={t("pricing.title")} actions={brand !== null ? <BrandSelect brands={items.map((i) => i.brand_id)} value={brand} onChange={choose} /> : undefined}>
      <Settled loading={!ready && failure === undefined} skeleton={<Skeleton className="h-64 w-full" />} className="flex flex-col gap-4">
        {failure !== undefined && <ErrorState failure={failure} onRetry={all.reload} />}
        {ready && brand === null && <EmptyState title={t("pricing.noBrands")} description={t("pricing.noBrands.body")} />}
        {brand !== null && <BrandPricing key={brand} brand={brand} today={today} />}
      </Settled>
    </ScreenFrame>
  );
}
