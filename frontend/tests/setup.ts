import { settleHost } from "@/shared/config/host";

// What a host's attributes would settle: the module-level `http` the entities call joins its paths onto this.
settleHost({ api: "/api/sites", base: "/sites", csrfCookie: "sa_csrf" });

// The tests run in Node: a write reads the CSRF cookie off `document.cookie` and finds none.
Object.assign(globalThis, { document: { cookie: "" } });
