import { Skeleton, Toaster } from "@evinvest/uikit";
import { type ComponentType, Suspense, lazy, useEffect } from "react";

import { may, useMe } from "@/entities/session";
import type { Me, Permission } from "@/shared/api";
import { host } from "@/shared/config/host";
import { useT } from "@/shared/i18n";
import { type Route, navigate, placeOf, usePathname } from "@/shared/lib/router";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { EmptyState } from "@/shared/ui/empty-state";

// One chunk per screen: the nav, on every page of the host, loads none of them.
const SCREENS: Record<Route, { view: ComponentType; needs: Permission }> = {
  leads: { view: lazy(() => import("@/views/leads").then((m) => ({ default: m.LeadsView }))), needs: "sa:sites:work:read" },
  places: { view: lazy(() => import("@/views/places").then((m) => ({ default: m.PlacesView }))), needs: "sa:sites:work:read" },
  pricing: { view: lazy(() => import("@/views/pricing").then((m) => ({ default: m.PricingView }))), needs: "sa:sites:work:read" },
  invoices: { view: lazy(() => import("@/views/invoices").then((m) => ({ default: m.InvoicesView }))), needs: "sa:sites:work:read" },
  overview: { view: lazy(() => import("@/views/overview").then((m) => ({ default: m.OverviewView }))), needs: "sa:sites:analysis:read" },
  experiments: { view: lazy(() => import("@/views/experiments").then((m) => ({ default: m.ExperimentsView }))), needs: "sa:sites:analysis:read" },
};

/** Where `base` itself leads (spec §10): the queue of new leads for whoever works them, else the funnel. */
function startOf(me: Me, base: string): string | null {
  if (may(me, "sa:sites:work:read")) return `${base}/leads?stage=created`;
  return may(me, "sa:sites:analysis:read") ? `${base}/overview` : null;
}

export const PageSkeleton = () => <Skeleton className="m-6 h-64" />;

/** The sites page under the host's `base`: one screen per route, by the caller's permissions. */
export function SitesPanel() {
  const t = useT();
  const me = useMe();
  const { base } = host();
  const place = placeOf(usePathname(), base);
  const start = startOf(me, base);
  const isDesktop = useMediaQuery(DESKTOP_QUERY);

  useEffect(() => {
    if (place.kind === "start" && start !== null) navigate(start, { replace: true });
  }, [place.kind, start]);

  const noAccess = <EmptyState className="m-6" title={t("state.noSection.title")} description={t("state.noSection.body")} />;
  let page;
  switch (place.kind) {
    // The host is leaving: its router changed the URL before it unmounts this element.
    case "outside":
      return null;
    case "start":
      page = start === null ? noAccess : null;
      break;
    case "unknown":
      page = <EmptyState className="m-6" title={t("state.notFound.title")} description={t("state.notFound.body")} />;
      break;
    case "route": {
      const { view: View, needs } = SCREENS[place.route];
      page = may(me, needs) ? (
        <Suspense fallback={<PageSkeleton />}>
          <View />
        </Suspense>
      ) : (
        noAccess
      );
    }
  }
  return (
    <>
      {page}
      {/* On a phone the bottom is the sheets; toasts come from the top. */}
      <Toaster position={isDesktop ? "bottom-right" : "top-center"} />
    </>
  );
}
