import type { NavIcon } from "@evinvest/uikit";
import { FlaskConical, Inbox, LayoutDashboard, MapPin, Receipt, Tag } from "lucide-react";

import type { Permission } from "@/shared/api/schema";
import type { T } from "@/shared/i18n";
import type { Route } from "@/shared/lib/router";

export interface NavEntry {
  route: Route;
  href: string;
  label: string;
  icon: NavIcon;
  description: string;
  current: boolean;
}

export interface NavSection {
  id: "work" | "analysis";
  label: string;
  items: NavEntry[];
}

/** The Sites dropdown: a section per permission that opens it. */
export function sitesNav(t: T, permissions: readonly Permission[], base: string, current: Route | null): NavSection[] {
  const entry = (route: Route, label: string, icon: NavIcon, description: string): NavEntry => ({
    route,
    href: `${base}/${route}`,
    label,
    icon,
    description,
    current: route === current,
  });
  const sections: NavSection[] = [];
  if (permissions.includes("sa:sites:work:read")) {
    sections.push({
      id: "work",
      label: t("nav.group.work"),
      items: [
        entry("leads", t("nav.leads"), Inbox, t("nav.leads.description")),
        entry("places", t("nav.places"), MapPin, t("nav.places.description")),
        entry("pricing", t("nav.pricing"), Tag, t("nav.pricing.description")),
        entry("invoices", t("nav.invoices"), Receipt, t("nav.invoices.description")),
      ],
    });
  }
  if (permissions.includes("sa:sites:analysis:read")) {
    sections.push({
      id: "analysis",
      label: t("nav.group.analysis"),
      items: [
        entry("overview", t("nav.overview"), LayoutDashboard, t("nav.overview.description")),
        entry("experiments", t("nav.experiments"), FlaskConical, t("nav.experiments.description")),
      ],
    });
  }
  return sections;
}
