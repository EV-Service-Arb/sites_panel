import { NavBadge, NavDot, NavigationMenuLink, SectionLabel } from "@evinvest/uikit";
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
  const place = placeOf(usePathname(), base);
  const current = place.kind === "route" ? place.route : null;
  const marks = useMarks(me.sub, current, may(me, "sa:sites:work:read"));

  useEffect(() => tellHost("sa:title-count", { count: marks.away }), [marks.away]);
  useEffect(() => () => tellHost("sa:title-count", { count: 0 }), []);

  return (
    // Wrapping, not a breakpoint: a phone's Sheet is narrower than two columns, the desktop dropdown sizes to them.
    <nav aria-label={t("nav.sites")} className="flex flex-wrap gap-x-6 gap-y-4 p-2">
      {sitesNav(t, me.permissions, marks, base, current).map((section) => (
        <section key={section.id} className="flex min-w-44 flex-col gap-1">
          <SectionLabel className="px-2">{section.label}</SectionLabel>
          {section.items.map((item) => (
            <NavigationMenuLink key={item.route} href={item.href} data-active={item.current} aria-current={item.current ? "page" : undefined} className="flex-row items-center justify-between gap-3">
              {item.label}
              {item.badge?.kind === "count" && <NavBadge count={item.badge.count} label={(n) => t("nav.badge", { n })} />}
              {item.badge?.kind === "dot" && <NavDot label={item.badge.label} />}
            </NavigationMenuLink>
          ))}
        </section>
      ))}
    </nav>
  );
}
