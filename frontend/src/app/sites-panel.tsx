import { mayOpen } from "@evinvest/views";
import { Skeleton, Toaster } from "@evinvest/uikit";
import { type ComponentType, Suspense, lazy, useEffect } from "react";

import { useMe } from "@/entities/session";
import type { Me } from "@/shared/api/schema";
import { useT } from "@/shared/i18n";
import { type ViewAt, type ViewName, openView, usePlace } from "@/shared/lib/router";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { VIEWS } from "@/shared/lib/views.generated";
import { EmptyState } from "@/shared/ui/empty-state";

// One chunk per screen: the nav, on every page of the host, loads none of them.
const SCREENS: Record<Exclude<ViewName, "start">, ComponentType> = {
  leads: lazy(() => import("@/views/leads").then((m) => ({ default: m.LeadsView }))),
  places: lazy(() => import("@/views/places").then((m) => ({ default: m.PlacesView }))),
  pricing: lazy(() => import("@/views/pricing").then((m) => ({ default: m.PricingView }))),
  invoices: lazy(() => import("@/views/invoices").then((m) => ({ default: m.InvoicesView }))),
  overview: lazy(() => import("@/views/overview").then((m) => ({ default: m.OverviewView }))),
  experiments: lazy(() => import("@/views/experiments").then((m) => ({ default: m.ExperimentsView }))),
};

/** Where `base` itself leads (spec §10): the queue of new leads for whoever works them, else the funnel. */
function startOf(me: Me): ViewAt | null {
  if (mayOpen(VIEWS, "leads", me.permissions)) return { view: "leads", params: { stage: "created" } };
  return mayOpen(VIEWS, "overview", me.permissions) ? { view: "overview", params: {} } : null;
}

export const PageSkeleton = () => <Skeleton className="m-6 h-64" />;

/** The sites page under the host's `base`: one screen per view, by the caller's permissions. */
export function SitesPanel() {
  const t = useT();
  const me = useMe();
  const place = usePlace();
  const start = startOf(me);
  const atStart = place.kind === "view" && place.at.view === "start";
  const isDesktop = useMediaQuery(DESKTOP_QUERY);

  useEffect(() => {
    if (atStart && start !== null) openView(start, { replace: true });
  }, [atStart, start]);

  const noAccess = <EmptyState className="m-6" title={t("state.noSection.title")} description={t("state.noSection.body")} />;
  let page;
  switch (place.kind) {
    // The host is leaving: its router changed the URL before it unmounts this element.
    case "outside":
      return null;
    case "unknown":
      page = <EmptyState className="m-6" title={t("state.notFound.title")} description={t("state.notFound.body")} />;
      break;
    case "view": {
      const { view } = place.at;
      if (view === "start") {
        page = start === null ? noAccess : null;
        break;
      }
      const Screen = SCREENS[view];
      page = mayOpen(VIEWS, view, me.permissions) ? (
        <Suspense fallback={<PageSkeleton />}>
          <Screen />
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
