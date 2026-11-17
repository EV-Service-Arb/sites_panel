import { useResource } from "@evinvest/api/react";
import { Settled, Skeleton } from "@evinvest/uikit";
import { ArrowUpRight } from "lucide-react";

import { wholeOf } from "@/entities/funnel";
import { brandsOf, usePlaces } from "@/entities/place";
import { FunnelFilters, useFilterParams } from "@/features/funnel-filters";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { ErrorState } from "@/shared/ui/error-state";
import { ScreenFrame } from "@/shared/ui/screen-frame";

import { LeadsBlock } from "./leads-block";

export function OverviewView() {
  const t = useT();
  const { period, brand, range, update } = useFilterParams();
  const gmb = gmbParam(useGmbs());
  const places = usePlaces();
  const funnel = useResource(api(), "/funnel", { params: { query: { ...range, brand, gmb } } });

  return (
    <ScreenFrame title={t("nav.overview")} actions={<FunnelFilters period={period} brand={brand} brands={brandsOf(places, brand)} onChange={update} />}>
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
