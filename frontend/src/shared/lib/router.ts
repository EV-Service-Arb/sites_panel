import { delegatedHref, dispatchNavigate } from "@evinvest/mfe";
import { useMemo, useSyncExternalStore } from "react";

import { host } from "@/shared/config/host";

const ROUTES = ["leads", "places", "pricing", "invoices", "overview", "experiments"] as const;
export type Route = (typeof ROUTES)[number];

/** Where a path is relative to `base`: its root, one of the routes, another path under it, or not under it at all. */
export type Place = { kind: "start" } | { kind: "route"; route: Route } | { kind: "unknown" } | { kind: "outside" };

export function placeOf(pathname: string, base: string): Place {
  if (pathname !== base && !pathname.startsWith(`${base}/`)) return { kind: "outside" };
  const rest = pathname.slice(base.length).replace(/\/+$/, "");
  if (rest === "") return { kind: "start" };
  const route = ROUTES.find((r) => rest === `/${r}`);
  return route === undefined ? { kind: "unknown" } : { kind: "route", route };
}

// One location for both elements: a nav click must re-render a mounted panel. The host's own router pushes
// state nobody hears, so a panel mounting or leaving is announced too (the nav's current item follows it).
const listeners = new Set<() => void>();
let popstate = false;
let panels = 0;

function notify(): void {
  for (const l of [...listeners]) l();
}

function subscribe(listener: () => void): () => void {
  if (!popstate) {
    popstate = true;
    window.addEventListener("popstate", notify);
  }
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function usePathname(): string {
  return useSyncExternalStore(subscribe, () => window.location.pathname);
}

export function useQuery(): URLSearchParams {
  const search = useSyncExternalStore(subscribe, () => window.location.search);
  return useMemo(() => new URLSearchParams(search), [search]);
}

export function navigate(href: string, { replace }: { replace: boolean }): void {
  if (replace) window.history.replaceState(null, "", href);
  else {
    window.history.pushState(null, "", href);
    window.scrollTo(0, 0);
  }
  notify();
}

/** The query of the page as it is, replaced: filters, the open card. */
export function replaceQuery(next: URLSearchParams): void {
  const query = next.toString();
  navigate(query === "" ? window.location.pathname : `${window.location.pathname}?${query}`, { replace: true });
}

/** A panel connected; the returned function is its disconnect. */
export function panelMounted(): () => void {
  panels += 1;
  notify();
  return () => {
    panels -= 1;
    notify();
  };
}

/**
 * A plain link click inside `root`: routed here when a panel is on the page to show it, otherwise offered to
 * the host's router (`ev:navigate` on window), and left to the browser when the host does not take it.
 */
export function followLink(e: MouseEvent, root: HTMLElement): void {
  if (!(e.target instanceof Element)) return;
  const anchor = e.target.closest("a[href]");
  if (!(anchor instanceof HTMLAnchorElement) || !root.contains(anchor)) return;
  const href = delegatedHref(e, anchor, window.location);
  if (href === null) return;
  if (panels > 0 && placeOf(new URL(href, window.location.origin).pathname, host().base).kind !== "outside") {
    e.preventDefault();
    navigate(href, { replace: false });
  } else if (dispatchNavigate(window, href)) e.preventDefault();
}
