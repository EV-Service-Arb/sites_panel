import { settleHost } from "@/shared/config/host";

// What a host's attributes would settle: `api()` joins the document's paths onto it.
settleHost({ api: "/api/sites", base: "/sites", csrfCookie: "sa_csrf" });

// The tests run in Node: a write reads the CSRF cookie off `document.cookie` and finds none.
Object.assign(globalThis, { document: { cookie: "" } });

// A browser resolves the API's root-relative URLs against the page; Node has no page, so this one stands in.
const ORIGIN = "http://sites.test";
class PageRequest extends Request {
  constructor(input: RequestInfo | URL, init?: RequestInit) {
    super(typeof input === "string" ? new URL(input, ORIGIN) : input, init);
  }
}
Object.assign(globalThis, { Request: PageRequest, location: new URL(`${ORIGIN}/sites`) });
