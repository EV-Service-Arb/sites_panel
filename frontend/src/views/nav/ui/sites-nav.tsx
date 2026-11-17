import { dispatchKeymap } from "@evinvest/mfe";
import { type NavGroup, NavMenuList } from "@evinvest/uikit";
import { type ReactNode, useEffect } from "react";

import { useMe } from "@/entities/session";
import { api } from "@/shared/api";
import { host } from "@/shared/config/host";
import { useT } from "@/shared/i18n";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { type Route, goTo, placeOf, usePathname } from "@/shared/lib/router";

import { sitesNav } from "../model/items";
import { ExperimentsMark, LeadsMark, PlacesMark, SelectedExperimentsMark } from "./marks";

const KEYS: Record<Route, string> = { leads: "l", places: "p", pricing: "r", invoices: "i", overview: "o", experiments: "e" };

/**
 * The Sites dropdown's content in the host's top bar; the host keeps it mounted, so it also counts the tab title
 * and binds a key to each view, under the scope the host assigns it.
 */
export function SitesNav({ element }: { element: HTMLElement }) {
  const t = useT();
  const me = useMe();
  const { base } = host();
  const pathname = usePathname();
  const place = placeOf(pathname, base);
  const current = place.kind === "route" ? place.route : null;
  const gmb = gmbParam(useGmbs());
  // Each mark reads what it counts, mounted only where the caller may read it; another selection starts them over.
  const reads = (path: "/leads/counts" | "/bookings/unmatched" | "/places" | "/experiments") => api().mayCall("get", path, me.permissions);
  const selection = gmb ?? "*";
  const badge = (route: Route): ReactNode => {
    switch (route) {
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

  const sections = sitesNav(t, me.permissions, base, current);
  const views = sections.flatMap((s) => s.items.map((e) => [e.route, e.label, e.href] as const));
  const viewsShape = JSON.stringify(views);
  useEffect(() => {
    const bound = new AbortController();
    for (const [route, label, href] of JSON.parse(viewsShape) as typeof views)
      dispatchKeymap(element, { on: { key: KEYS[route] }, id: route, run: { label, run: () => goTo(href) } }, bound.signal);
    return () => bound.abort();
  }, [element, viewsShape]);

  const groups: NavGroup[] = sections.map((section) => ({
    id: section.id,
    label: section.label,
    items: section.items.map((e) => ({
      id: e.route,
      href: e.href,
      label: e.label,
      icon: e.icon,
      description: e.description,
      match: () => e.current,
      badge: badge(e.route),
    })),
  }));
  return (
    <nav aria-label={t("nav.sites")}>
      <NavMenuList groups={groups} pathname={pathname} badgeLabel={(n) => t("nav.badge", { n })} />
    </nav>
  );
}
