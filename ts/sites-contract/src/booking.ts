import { BOOKING_LIMITS as LIMIT, BOOKING_PROVIDERS, DAY_PARTS, PAGE_PROVIDERS, type DayPart } from "./generated/booking.js";
import type { BookingCode } from "./generated/codes.js";
import type { BookingConfig, BookingProvider, PageProvider } from "./generated/types.js";
import type { BookingRequestedV1 } from "./generated/wire.js";
import { at, isDate, isObject } from "./text.js";

/** One thing wrong with a place's `booking`: where below it (`""`, `default`, `providers.cal_com.url`) and why. */
export interface BookingProblem {
  path: string;
  code: BookingCode;
}

export const isBookingProvider = (v: unknown): v is BookingProvider => BOOKING_PROVIDERS.some(p => p === v);
export const isPageProvider = (v: unknown): v is PageProvider => PAGE_PROVIDERS.some(p => p === v);

const LEAD_REF = new RegExp(LIMIT.leadRef);
const CAL_SEGMENT = new RegExp(LIMIT.calComSegment);
const LABEL = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/;
const APPOINTMENTS = "/calendar/appointments/";

/** `lead-<row>-<8 lowercase hex>`, the row 1 or more: the panel's lead id for a site's lead. */
export const isLeadRef = (v: unknown): v is string => typeof v === "string" && LEAD_REF.test(v);

/**
 * Why `url` may not be `provider`'s booking page, or `null` (`contracts/vectors/booking/README.md`,
 * normative): at most 2048 printable ASCII; the literal `https://`; no `#` and no `\`; an
 * authority without `@`, `:` or `[`; a dotted DNS name whose last label is not a number; then
 * the provider's own rule.
 */
export function checkBookingUrl(provider: PageProvider, url: string): BookingCode | null {
  if (url.length === 0 || url.length > LIMIT.maxUrl || !/^[\x21-\x7e]+$/.test(url)) return "format";
  if (!url.startsWith("https://")) return "scheme";
  if (url.includes("#")) return "fragment";
  if (url.includes("\\")) return "backslash";
  const rest = url.slice("https://".length);
  const split = rest.search(/[/?]/);
  const host = split < 0 ? rest : rest.slice(0, split);
  if (host.includes("@")) return "userinfo";
  if (host.startsWith("[")) return "ip";
  if (host.includes(":")) return "port";
  const labels = host.split(".");
  if (host.length > 253 || labels.length < 2 || !labels.every(l => LABEL.test(l))) return "host";
  const last = (labels.at(-1) ?? "").toLowerCase();
  // A WHATWG parser reads a host whose last label is a number as an IPv4 address.
  if (/^\d+$/.test(last) || last.startsWith("0x")) return "ip";
  const name = host.toLowerCase();
  const path = (split < 0 ? "" : rest.slice(split)).split("?")[0] ?? "";
  switch (provider) {
    case "link":
      return null;
    case "google_calendar":
      return (name === "calendar.app.google" && path.length > 1) || (name === "calendar.google.com" && path.length > APPOINTMENTS.length && path.startsWith(APPOINTMENTS)) ? null : "google";
    case "cal_com": {
      if (!LIMIT.calComHosts.some(h => h === name)) return "cal_host";
      const segments = path.replace(/^\//, "").split("/");
      return path.startsWith("/") && segments.length === 2 && segments.every(s => CAL_SEGMENT.test(s)) ? null : "cal_path";
    }
  }
}

/**
 * A place's `booking` checked whole: every problem, and the config when there is none. `manual`
 * is always available, so never a key of `providers`; the default is `manual` or one of them.
 */
export function checkBookingConfig(v: unknown): { problems: BookingProblem[]; config: BookingConfig | null } {
  const problems: BookingProblem[] = [];
  const fail = (path: string, code: BookingCode) => problems.push({ path, code });
  if (!isObject(v)) return { problems: [{ path: "", code: "shape" }], config: null };
  if (Object.keys(v).some(k => k !== "default" && k !== "providers")) fail("", "unknown_key");
  const def = isBookingProvider(v.default) ? v.default : null;
  if (!Object.hasOwn(v, "default")) fail("default", "required");
  else if (def === null) fail("default", "default_unknown");
  const providers: BookingConfig["providers"] = {};
  if (!Object.hasOwn(v, "providers")) fail("providers", "required");
  else if (!isObject(v.providers)) fail("providers", "shape");
  else
    for (const [name, conf] of Object.entries(v.providers)) {
      const path = at("providers", name);
      if (!isBookingProvider(name)) fail(path, "provider_unknown");
      else if (!isObject(conf)) fail(path, "provider_shape");
      else if (Object.keys(conf).some(k => k !== "url")) fail(path, "provider_extra");
      else if (!isPageProvider(name)) fail(path, "manual");
      else if (!Object.hasOwn(conf, "url")) fail(at(path, "url"), "required");
      else if (typeof conf.url !== "string") fail(at(path, "url"), "url_type");
      else {
        const bad = checkBookingUrl(name, conf.url);
        if (bad) fail(at(path, "url"), bad);
        else providers[name] = { url: conf.url };
      }
    }
  // A default whose own entry is refused is blamed there, not twice.
  if (isPageProvider(def) && !Object.hasOwn(providers, def) && !problems.some(p => p.path.startsWith(at("providers", def)))) fail("default", "default_unset");
  return problems.length > 0 || def === null ? { problems, config: null } : { problems, config: { default: def, providers } };
}

/** `booking.requested@1`'s properties, their vocabularies as the panel checks them. */
export type BookingRequested = BookingRequestedV1 & { provider: BookingProvider; preferred_part?: DayPart };

const isDayPart = (v: unknown): v is DayPart => DAY_PARTS.some(p => p === v);

/**
 * `booking.requested@1`'s properties, or `null` when the panel would refuse them: a lead ref, a
 * provider and, with `manual` only, a day and a part of the day. No null, nothing else, no free
 * text.
 */
export function bookingRequested(v: unknown): BookingRequested | null {
  if (!isObject(v) || Object.entries(v).some(([k, x]) => x === null || !["lead_ref", "provider", "preferred_date", "preferred_part"].includes(k))) return null;
  const { lead_ref, provider, preferred_date, preferred_part } = v;
  if (!isLeadRef(lead_ref) || !isBookingProvider(provider)) return null;
  if ((preferred_date !== undefined && !isDate(preferred_date)) || (preferred_part !== undefined && !isDayPart(preferred_part))) return null;
  if (provider !== "manual" && (preferred_date !== undefined || preferred_part !== undefined)) return null;
  return {
    lead_ref,
    provider,
    ...(preferred_date !== undefined ? { preferred_date } : {}),
    ...(preferred_part !== undefined ? { preferred_part } : {}),
  };
}

const DAY_MS = 86_400_000;

/**
 * Whether a visitor's preferred day may be asked for on `today` (both `YYYY-MM-DD`): two days
 * back (clocks, an outbox's delay) to a year and a day ahead. The panel checks it on arrival.
 */
export function preferredDateInWindow(day: string, today: string): boolean {
  const days = (Date.parse(`${day}T00:00:00Z`) - Date.parse(`${today}T00:00:00Z`)) / DAY_MS;
  return days >= -2 && days <= 366;
}
