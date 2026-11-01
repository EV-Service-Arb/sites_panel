import { API_VERSION } from "@evinvest/mfe";
import { PortalProvider } from "@evinvest/uikit";
import { type ReactNode, useLayoutEffect } from "react";
import { type Root, createRoot } from "react-dom/client";

import { settleHost } from "@/shared/config/host";
import { LocaleProvider, localeOf } from "@/shared/i18n";
import { followHostLive } from "@/shared/lib/live";
import { followLink } from "@/shared/lib/router";

/** Sets `[data-ready]` in the commit that first paints the element, the signal hosts wait on (concierge's define-element). */
function ReadyMark({ host }: { host: HTMLElement }) {
  useLayoutEffect(() => {
    host.toggleAttribute("data-ready", true);
  }, [host]);
  return null;
}

// Once per page, as a `<link>` next to the entry: the host moves it under its own layers.
let stylesheet = false;
function attachStylesheet(): void {
  if (stylesheet) return;
  stylesheet = true;
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = new URL("./mfe-sites.css", import.meta.url).href;
  document.head.appendChild(link);
}

function attribute(el: HTMLElement, name: string): string {
  const value = el.getAttribute(name);
  if (value === null) throw new Error(`<${el.localName}> needs its ${name} attribute`);
  return value;
}

/**
 * Registers `tag` as a self-rendering element (concierge web/src/shared/element/define-element.tsx): React in its
 * LIGHT DOM, overlays portaled into it, the locale off the closest `[lang]`, `[data-ready]` once the first render
 * committed. `api`, `base` and `csrf-cookie` are read on connect; the host sets them before.
 */
export function defineElement(tag: string, render: () => ReactNode, onConnect: () => () => void = () => () => {}): void {
  class SitesElement extends HTMLElement {
    static apiVersion = API_VERSION;
    #root: Root | undefined;
    #lang: MutationObserver | undefined;
    #disconnect: (() => void) | undefined;
    #click = (e: MouseEvent) => followLink(e, this);

    connectedCallback() {
      // Light DOM: a host's PostHog autocapture and session replay would otherwise record the leads' PII shown here.
      this.classList.add("ph-no-capture");
      settleHost({ api: attribute(this, "api"), base: attribute(this, "base"), csrfCookie: attribute(this, "csrf-cookie") });
      attachStylesheet();
      followHostLive();
      const langHost = this.closest("[lang]");
      if (langHost === null) throw new Error(`<${tag}> is under no [lang]`);
      this.addEventListener("click", this.#click);
      this.#disconnect = onConnect();
      this.#root = createRoot(this);
      const paint = () => {
        this.#root?.render(
          <LocaleProvider value={localeOf(langHost.getAttribute("lang") ?? undefined)}>
            <PortalProvider container={this}>
              {render()}
              <ReadyMark host={this} />
            </PortalProvider>
          </LocaleProvider>,
        );
      };
      this.#lang = new MutationObserver(paint);
      this.#lang.observe(langHost, { attributeFilter: ["lang"] });
      paint();
    }

    disconnectedCallback() {
      this.#lang?.disconnect();
      this.#disconnect?.();
      this.removeEventListener("click", this.#click);
      this.#root?.unmount();
      this.#root = undefined;
      this.removeAttribute("data-ready");
    }
  }
  if (!customElements.get(tag)) customElements.define(tag, SitesElement);
}
