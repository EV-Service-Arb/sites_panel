import { NavDot, type NavGroup, NavMenuList } from "@evinvest/uikit";
import { useEffect } from "react";

import { may, useMe } from "@/entities/session";
import { host } from "@/shared/config/host";
import { useT } from "@/shared/i18n";
import { tellHost } from "@/shared/lib/host-events";
import { placeOf, usePathname } from "@/shared/lib/router";

import { sitesNav } from "../model/items";
import { useMarks } from "../model/use-marks";

/** The Sites dropdown's content in the host's top bar; the host keeps it mounted, so it also counts the tab title. */
export function SitesNav() {
  const t = useT();
  const me = useMe();
  const { base } = host();
  const pathname = usePathname();
  const place = placeOf(pathname, base);
  const current = place.kind === "route" ? place.route : null;
  const marks = useMarks(me.sub, current, may(me, "sa:sites:work:read"));

  useEffect(() => tellHost("sa:title-count", { count: marks.away }), [marks.away]);
  useEffect(() => () => tellHost("sa:title-count", { count: 0 }), []);

  const groups: NavGroup[] = sitesNav(t, me.permissions, marks, base, current).map((section) => ({
    id: section.id,
    label: section.label,
    items: section.items.map((e) => ({
      id: e.route,
      href: e.href,
      label: e.label,
      icon: e.icon,
      description: e.description,
      match: () => e.current,
      ...(e.badge?.kind === "count" && { badge: e.badge.count }),
      ...(e.badge?.kind === "dot" && { badge: <NavDot label={e.badge.label} /> }),
    })),
  }));
  return (
    <nav aria-label={t("nav.sites")}>
      <NavMenuList groups={groups} pathname={pathname} badgeLabel={(n) => t("nav.badge", { n })} />
    </nav>
  );
}
