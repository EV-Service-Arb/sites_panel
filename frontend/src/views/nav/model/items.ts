import type { NavIcon } from "@evinvest/uikit";
import { mayOpen } from "@evinvest/views";
import { FlaskConical, Inbox, LayoutDashboard, MapPin, Receipt, Tag } from "lucide-react";

import type { Permission } from "@/shared/api/schema";
import type { T } from "@/shared/i18n";
import { type ViewName, hrefOf } from "@/shared/lib/router";
import { VIEWS } from "@/shared/lib/views.generated";

type Views = (typeof VIEWS)["views"];
/** The views with a key of their own: the ones the nav lists. */
export type NavView = { [N in ViewName]: Views[N] extends { key: string } ? N : never }[ViewName];
type Needs = Views[NavView]["needs"];

/** Each listed view's icon, in the nav's order. */
const ICONS: Record<NavView, NavIcon> = { leads: Inbox, places: MapPin, pricing: Tag, invoices: Receipt, overview: LayoutDashboard, experiments: FlaskConical };

export interface NavEntry {
  view: NavView;
  key: string;
  href: string;
  label: string;
  icon: NavIcon;
  description: string;
  current: boolean;
}

export interface NavSection {
  /** The permission that opens the section's views. */
  id: Needs;
  label: string;
  items: NavEntry[];
}

/** The Sites dropdown: a section per permission the views need, those the caller holds. */
export function sitesNav(t: T, permissions: readonly Permission[], current: ViewName | null): NavSection[] {
  const sections: NavSection[] = [];
  for (const view of Object.keys(ICONS) as NavView[]) {
    if (!mayOpen(VIEWS, view, permissions)) continue;
    const { needs, key } = VIEWS.views[view];
    let section = sections.find((s) => s.id === needs);
    if (section === undefined) {
      section = { id: needs, label: t(`nav.group.${needs}`), items: [] };
      sections.push(section);
    }
    section.items.push({ view, key, href: hrefOf({ view, params: {} }), label: t(`nav.${view}`), icon: ICONS[view], description: t(`nav.${view}.description`), current: view === current });
  }
  return sections;
}
