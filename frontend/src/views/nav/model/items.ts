import type { Permission } from "@/shared/api";
import type { T } from "@/shared/i18n";
import type { Route } from "@/shared/lib/router";

import type { Marks } from "./use-marks";

export type Badge = { kind: "count"; count: number } | { kind: "dot"; label: string } | null;

export interface NavEntry {
  route: Route;
  href: string;
  label: string;
  badge: Badge;
  current: boolean;
}

export interface NavSection {
  id: "work" | "analysis";
  label: string;
  items: NavEntry[];
}

const count = (n: number): Badge => (n > 0 ? { kind: "count", count: n } : null);

/**
 * The Sites dropdown: a section per permission that opens it. New leads are counted; bookings without a
 * lead only say "something waits", and a count says it already.
 */
export function sitesNav(t: T, permissions: readonly Permission[], marks: Marks, base: string, current: Route | null): NavSection[] {
  const entry = (route: Route, label: string, badge: Badge): NavEntry => ({ route, href: `${base}/${route}`, label, badge, current: route === current });
  const leadsBadge = marks.leads === 0 && marks.bookings ? { kind: "dot" as const, label: t("nav.unmatched") } : count(marks.leads);
  const sections: NavSection[] = [];
  if (permissions.includes("sa:sites:work:read")) {
    sections.push({
      id: "work",
      label: t("nav.group.work"),
      items: [
        entry("leads", t("nav.leads"), leadsBadge),
        entry("places", t("nav.places"), count(marks.places)),
        entry("pricing", t("nav.pricing"), null),
        entry("invoices", t("nav.invoices"), null),
      ],
    });
  }
  if (permissions.includes("sa:sites:analysis:read")) {
    sections.push({
      id: "analysis",
      label: t("nav.group.analysis"),
      items: [entry("overview", t("nav.overview"), null), entry("experiments", t("nav.experiments"), marks.experiments ? { kind: "dot", label: t("nav.changed") } : null)],
    });
  }
  return sections;
}
