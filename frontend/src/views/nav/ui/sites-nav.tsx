import { dispatchKeymap } from "@evinvest/mfe";
import { type NavGroup, NavMenuList } from "@evinvest/uikit";
import { type ReactNode, useEffect } from "react";

import { useMe } from "@/entities/session";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { goTo, usePathname, usePlace } from "@/shared/lib/router";

import { type NavView, sitesNav } from "../model/items";
import { ExperimentsMark, LeadsMark, PlacesMark, SelectedExperimentsMark } from "./marks";

/**
 * The Sites dropdown's content in the host's top bar; the host keeps it mounted, so it also counts the tab title
 * and binds a key to each view, under the scope the host assigns it.
 */
export function SitesNav({ element }: { element: HTMLElement }) {
  const t = useT();
  const me = useMe();
  const pathname = usePathname();
  const place = usePlace();
  const current = place.kind === "view" ? place.at.view : null;
  const gmb = gmbParam(useGmbs());
  // Each mark reads what it counts, mounted only where the caller may read it; another selection starts them over.
  const reads = (path: "/leads/counts" | "/bookings/unmatched" | "/places" | "/experiments") => api().mayCall("get", path, me.permissions);
  const selection = gmb ?? "*";
  const badge = (view: NavView): ReactNode => {
    switch (view) {
      case "leads":
        return reads("/leads/counts") && reads("/bookings/unmatched") && reads("/places") ? <LeadsMark key={selection} userId={me.sub} open={current === "leads"} /> : null;
      case "places":
        return reads("/places") ? <PlacesMark key={selection} open={current === "places"} /> : null;
      case "experiments":
        if (!reads("/experiments")) return null;
        if (reads("/places")) return <SelectedExperimentsMark key={selection} open={current === "experiments"} />;
        // Without the places, a selection cannot be narrowed to its brands: no mark rather than another selection's.
        return gmb === null ? <ExperimentsMark open={current === "experiments"} selected={null} /> : null;
      default:
        return null;
    }
  };

  const sections = sitesNav(t, me.permissions, current);
  const views = sections.flatMap((s) => s.items.map((e) => [e.view, e.key, e.label, e.href] as const));
  const viewsShape = JSON.stringify(views);
  useEffect(() => {
    const bound = new AbortController();
    for (const [view, key, label, href] of JSON.parse(viewsShape) as typeof views)
      dispatchKeymap(element, { on: { key }, id: view, run: { label, run: () => goTo(href) } }, bound.signal);
    return () => bound.abort();
  }, [element, viewsShape]);

  const groups: NavGroup[] = sections.map((section) => ({
    id: section.id,
    label: section.label,
    items: section.items.map((e) => ({
      id: e.view,
      href: e.href,
      label: e.label,
      icon: e.icon,
      description: e.description,
      match: () => e.current,
      badge: badge(e.view),
    })),
  }));
  return (
    <nav aria-label={t("nav.sites")}>
      <NavMenuList groups={groups} pathname={pathname} badgeLabel={(n) => t("nav.badge", { n })} />
    </nav>
  );
}
