import { useResource } from "@evinvest/api/react";
import { Settled, Skeleton } from "@evinvest/uikit";
import { ArrowUpRight } from "lucide-react";
import { useState } from "react";

import { wholeOf } from "@/entities/funnel";
import { brandsOf, usePlaces } from "@/entities/place";
import { FunnelFilters, type Period, rangeOf } from "@/features/funnel-filters";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { openView, useViewParams } from "@/shared/lib/router";
import { ErrorState } from "@/shared/ui/error-state";
import { ScreenFrame } from "@/shared/ui/screen-frame";

import { LeadsBlock } from "./leads-block";

export function OverviewView() {
  const t = useT();
  const { period, brand } = useViewParams("overview");
  // Fixed when the screen opens: a range that moved mid-render would read again in a loop.
  const [now] = useState(() => new Date());
  const range = period === undefined ? {} : rangeOf(Number(period), now);
  const update = ({ period: p = period, brand: b = brand }: { period?: Period; brand?: string | null }) =>
    openView({ view: "overview", params: { ...(p !== undefined && { period: p }), ...(b && { brand: b }) } }, { replace: true });
  const gmb = gmbParam(useGmbs());
  const places = usePlaces();
  const funnel = useResource(api(), "/funnel", { params: { query: { ...range, brand: brand ?? null, gmb } } });

  return (
    <ScreenFrame title={t("nav.overview")} actions={<FunnelFilters period={period} brand={brand} brands={brandsOf(places, brand ?? null)} onChange={update} />}>
      <Settled loading={funnel.status === "loading"} skeleton={<Skeleton className="h-96 w-full max-w-3xl" />} className="flex max-w-3xl flex-col gap-3">
        {funnel.status === "error" && <ErrorState failure={funnel.failure} onRetry={funnel.reload} />}
        {funnel.status === "ok" && (
          <>
            <LeadsBlock funnel={wholeOf(funnel.data)} />
            <p className="px-1 text-sm text-ink-soft" role="note">
              {t("funnel.site.posthog")}{" "}
              {funnel.data.posthog_url !== null && (
                <a href={funnel.data.posthog_url} target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-0.5 underline">
                  {t("funnel.site.posthogOpen")}
                  <ArrowUpRight aria-hidden className="size-3.5" />
                </a>
              )}{" "}
              {t("funnel.site.mapsPending")}
            </p>
            <p className="px-1 text-xs text-ink-soft">{t("funnel.range", { from: funnel.data.from, to: funnel.data.to })}</p>
          </>
        )}
      </Settled>
    </ScreenFrame>
  );
}
