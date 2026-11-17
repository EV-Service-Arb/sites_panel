import { delegatedHref, dispatchNavigate } from "@evinvest/mfe";
import { type At, ViewError, href, parse } from "@evinvest/views";
import { useMemo, useSyncExternalStore } from "react";

import { host } from "@/shared/config/host";

import { VIEWS } from "./views.generated";

export type ViewAt = At<typeof VIEWS>;
export type ViewName = ViewAt["view"];
export type ViewParams<N extends ViewName> = Extract<ViewAt, { view: N }>["params"];

/** Where an href is relative to `base`: one of the views, another href under it, or not under it at all. */
export type Place = { kind: "view"; at: ViewAt } | { kind: "unknown" } | { kind: "outside" };

const under = (pathname: string, base: string) => pathname === base || pathname.startsWith(`${base}/`);

export function placeOf(pathname: string, search: string, base: string): Place {
  if (!under(pathname, base)) return { kind: "outside" };
  try {
    return { kind: "view", at: parse(VIEWS, `${pathname.slice(base.length) || "/"}${search}`).at };
  } catch (e) {
    // An href that is no view is "nothing here", not a failure; anything else is.
    if (e instanceof ViewError) return { kind: "unknown" };
    throw e;
  }
}

/** `at` under the host's base. */
export function hrefOf(at: ViewAt): string {
  const below = href(VIEWS, at);
  return below === "/" ? host().base : `${host().base}${below}`;
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

export function usePlace(): Place {
  const pathname = usePathname();
  const search = useSyncExternalStore(subscribe, () => window.location.search);
  return useMemo(() => placeOf(pathname, search, host().base), [pathname, search]);
}

/** The params of view `name`, under the screen that shows it: elsewhere, a broken route table. */
export function useViewParams<N extends ViewName>(name: N): ViewParams<N> {
  const place = usePlace();
  if (place.kind !== "view" || place.at.view !== name) throw new Error(`${name}'s params asked off ${JSON.stringify(place)}`);
  return place.at.params as ViewParams<N>;
}

function navigate(href: string, { replace }: { replace: boolean }): void {
  if (replace) window.history.replaceState(null, "", href);
  else {
    window.history.pushState(null, "", href);
    window.scrollTo(0, 0);
  }
  notify();
}

/** To `at`: in place (`replace`, a filter, the open card), or as a new entry of the history. */
export function openView(at: ViewAt, { replace }: { replace: boolean }): void {
  navigate(hrefOf(at), { replace });
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

/** Routed here when a panel is on the page to show it, otherwise offered to the host's router (`ev:navigate` on window). Whether either took it. */
function route(href: string): boolean {
  if (panels > 0 && under(new URL(href, window.location.origin).pathname, host().base)) {
    navigate(href, { replace: false });
    return true;
  }
  return dispatchNavigate(window, href);
}

/** A plain link click inside `root`, routed; left to the browser when nothing takes it. */
export function followLink(e: MouseEvent, root: HTMLElement): void {
  if (!(e.target instanceof Element)) return;
  const anchor = e.target.closest("a[href]");
  if (!(anchor instanceof HTMLAnchorElement) || !root.contains(anchor)) return;
  const href = delegatedHref(e, anchor, window.location);
  if (href !== null && route(href)) e.preventDefault();
}

/** A same-origin path, routed as a link click is; loaded when nothing takes it. */
export function goTo(href: string): void {
  if (!route(href)) window.location.assign(href);
}
