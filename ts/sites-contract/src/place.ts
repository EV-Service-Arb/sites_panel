import { checkBookingConfig } from "./booking-config.js";
import type { BookingCode } from "./generated/booking-codes.js";
import type { PlaceCode } from "./generated/place-codes.js";
import { DAYS } from "./generated/days.js";
import { MESSENGERS } from "./generated/messengers.js";
import { PLACE_LIMITS as LIMIT } from "./generated/place-limits.js";
import type { Address, DayOfWeek, Geo, Messenger, OpeningHours, PlaceLive, Rating } from "./generated/types.js";
import { at, charCount, hasControl, hasWhitespace, isDate, isObject, trim, type Json } from "./text.js";

/** One thing wrong with a place's settings: its path (`phone`, `hours[0].opens`, `booking.providers.cal_com.url`) and why. */
export interface PlaceProblem {
  path: string;
  code: PlaceCode | BookingCode;
}

const E164 = /* @__PURE__ */ new RegExp(LIMIT.e164);
const TELEGRAM_BOT = /* @__PURE__ */ new RegExp(LIMIT.telegramBot);
const TIME = /* @__PURE__ */ new RegExp(LIMIT.time);

class Check {
  readonly problems: PlaceProblem[] = [];
  fail(path: string, code: PlaceCode | BookingCode): void {
    this.problems.push({ path, code });
  }
  /** `f`'s value when it added no problem. */
  clean<T>(f: () => T): T | undefined {
    const before = this.problems.length;
    const out = f();
    return this.problems.length === before ? out : undefined;
  }
  /** A line of text, trimmed: not blank, at most `max` characters, no control characters. */
  text(path: string, v: unknown, max: number): string {
    if (typeof v !== "string") {
      this.fail(path, "type");
      return "";
    }
    const s = trim(v);
    if (s === "") this.fail(path, "blank");
    else if (charCount(s) > max) this.fail(path, "too_long");
    else if (hasControl(s)) this.fail(path, "control");
    return s;
  }
  /** A text whose only fault is a `code` of its own: trimmed, and held to `ok`. */
  word(path: string, v: unknown, max: number, code: PlaceCode, ok: (s: string) => boolean): string {
    const s = typeof v === "string" ? trim(v) : "";
    if (s === "" || charCount(s) > max || hasControl(s) || !ok(s)) this.fail(path, code);
    return s;
  }
  /** An object with exactly `keys`; every other key and every missing one named. */
  object(path: string, v: unknown, keys: readonly string[]): Json | null {
    if (!isObject(v)) {
      this.fail(path, "shape");
      return null;
    }
    const before = this.problems.length;
    for (const k of Object.keys(v)) if (!keys.includes(k)) this.fail(at(path, k), "unknown_field");
    for (const k of keys) if (!Object.hasOwn(v, k)) this.fail(at(path, k), "required");
    return this.problems.length === before ? v : null;
  }
  /** A list of 1 to `max` entries. */
  list(path: string, v: unknown, max: number): unknown[] {
    if (!Array.isArray(v)) this.fail(path, "shape");
    else if (v.length === 0) this.fail(path, "empty");
    else if (v.length > max) this.fail(path, "too_many");
    else return v;
    return [];
  }
}

/** The host of an absolute `https://` URL without userinfo or whitespace, lowercase. */
function httpsHost(s: string): string | null {
  if (!s.startsWith("https://")) return null;
  const authority = s.slice("https://".length).split(/[/?#]/, 1)[0] ?? "";
  const ok = authority !== "" && !authority.startsWith(":") && !authority.startsWith(".") && !authority.includes("@") && !hasWhitespace(s);
  return ok ? authority.toLowerCase() : null;
}

/**
 * An RFC 3339 instant: `YYYY-MM-DDTHH:MM:SS`, an optional fraction of 1 to 9 digits, and `Z`
 * or `±HH:MM` — on a day the calendar has, at a time the clock has (no leap second), at an
 * offset of less than a day.
 */
export function isInstant(s: string): boolean {
  const m = /^(\d{4}-\d{2}-\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(?:Z|[+-](\d{2}):(\d{2}))$/.exec(s);
  if (!m || !isDate(m[1])) return false;
  const [hour, minute, second, offsetHours, offsetMinutes] = m.slice(2).map(n => (n === undefined ? 0 : Number(n)));
  return hour! <= 23 && minute! <= 59 && second! <= 59 && offsetHours! <= 23 && offsetMinutes! <= 59;
}

const isDay = (v: unknown): v is DayOfWeek => DAYS.some(d => d === v);
const isMessenger = (v: unknown): v is Messenger => MESSENGERS.some(m => m === v);

function hours(c: Check, path: string, v: unknown): OpeningHours[] {
  const out: OpeningHours[] = [];
  for (const [i, row] of c.list(path, v, LIMIT.maxHours).entries()) {
    const there = at(path, i);
    const m = c.object(there, row, ["days", "opens", "closes"]);
    if (!m) continue;
    const days = c.clean(() => {
      const seen: DayOfWeek[] = [];
      for (const [j, d] of c.list(at(there, "days"), m.days, DAYS.length).entries()) {
        if (!isDay(d)) c.fail(at(at(there, "days"), j), "day");
        else if (seen.includes(d)) c.fail(at(at(there, "days"), j), "duplicate");
        else seen.push(d);
      }
      return DAYS.filter(d => seen.includes(d));
    });
    const time = (key: "opens" | "closes") =>
      c.clean(() => {
        const t = m[key];
        if (typeof t === "string" && TIME.test(t)) return t;
        c.fail(at(there, key), "time");
        return "";
      });
    const opens = time("opens");
    const closes = time("closes");
    if (days === undefined || opens === undefined || closes === undefined) continue;
    // HH:MM sorts as the time it names.
    if (closes <= opens) c.fail(at(there, "closes"), "closes_before_opens");
    else out.push({ days, opens, closes });
  }
  return out;
}

function serviceArea(c: Check, path: string, v: unknown): string[] {
  const out: string[] = [];
  for (const [i, n] of c.list(path, v, LIMIT.maxArea).entries()) {
    const name = c.clean(() => c.text(at(path, i), n, LIMIT.maxText));
    if (name === undefined) continue;
    if (out.some(o => o.toLowerCase() === name.toLowerCase())) c.fail(at(path, i), "duplicate");
    else out.push(name);
  }
  return out;
}

function address(c: Check, path: string, v: unknown): Address | null {
  const m = c.object(path, v, ["street", "postalCode", "locality"]);
  if (!m) return null;
  return {
    street: c.text(at(path, "street"), m.street, LIMIT.maxText),
    postalCode: c.word(at(path, "postalCode"), m.postalCode, 5, "postal_code", p => /^\d{5}$/.test(p)),
    locality: c.text(at(path, "locality"), m.locality, LIMIT.maxText),
  };
}

function geo(c: Check, path: string, v: unknown): Geo | null {
  const m = c.object(path, v, ["lat", "lng"]);
  if (!m) return null;
  const coord = (key: "lat" | "lng", limit: number) => {
    const x = m[key];
    if (typeof x === "number" && Math.abs(x) <= limit) return x;
    c.fail(at(path, key), "coordinate");
    return 0;
  };
  return { lat: coord("lat", 90), lng: coord("lng", 180) };
}

function landmark(c: Check, path: string, v: unknown): Record<string, string> {
  if (!isObject(v)) {
    c.fail(path, "shape");
    return {};
  }
  const entries = Object.entries(v);
  if (entries.length === 0) c.fail(path, "empty");
  else if (entries.length > LIMIT.maxLocales) c.fail(path, "too_many");
  const out: Record<string, string> = {};
  for (const [locale, text] of entries) {
    if (!/^[a-z]{2}$/.test(locale)) c.fail(at(path, locale), "locale");
    else out[locale] = c.text(at(path, locale), text, LIMIT.maxText);
  }
  return out;
}

function rating(c: Check, path: string, v: unknown): Rating | null {
  const m = c.object(path, v, ["value", "count", "fetchedAt"]);
  if (!m) return null;
  const { value, count, fetchedAt } = m;
  const valueOk = typeof value === "number" && value >= 1 && value <= 5;
  const countOk = typeof count === "number" && Number.isSafeInteger(count) && count >= 0;
  const atOk = typeof fetchedAt === "string" && isInstant(fetchedAt);
  if (!valueOk) c.fail(at(path, "value"), "rating_value");
  if (!countOk) c.fail(at(path, "count"), "rating_count");
  if (!atOk) c.fail(at(path, "fetchedAt"), "instant");
  return valueOk && countOk && atOk ? { value, count, fetchedAt } : null;
}

function messengers(c: Check, path: string, v: unknown): Partial<Record<Messenger, boolean>> {
  if (!isObject(v)) {
    c.fail(path, "shape");
    return {};
  }
  const out: Partial<Record<Messenger, boolean>> = {};
  for (const [k, on] of Object.entries(v)) {
    if (!isMessenger(k)) c.fail(at(path, k), "messenger_unknown");
    else if (typeof on !== "boolean") c.fail(at(path, k), "boolean");
    else out[k] = on;
  }
  return out;
}

/**
 * A place's live settings checked field by field (`contracts/vectors/place.json`): the fields
 * that pass, and every problem of those that do not. The panel refuses settings with any
 * problem; a site keeps what passed and serves its baked config for the rest. A field set to
 * `null` is left out, as if absent.
 */
export function checkPlaceLive(body: unknown): { live: PlaceLive; problems: PlaceProblem[] } {
  const c = new Check();
  const live: PlaceLive = {};
  if (!isObject(body)) return { live, problems: [{ path: "", code: "object" }] };
  const set = <K extends keyof PlaceLive>(key: K, value: PlaceLive[K] | null | undefined) => {
    if (value !== undefined && value !== null) live[key] = value;
  };
  for (const [name, v] of Object.entries(body)) {
    if (v === null) continue;
    switch (name) {
      case "phone":
      case "whatsapp":
        set(name, c.clean(() => c.word(name, v, 16, "e164", s => E164.test(s))));
        break;
      case "telegram":
        set(name, c.clean(() => c.word(name, v, 32, "telegram_bot", s => TELEGRAM_BOT.test(s))));
        break;
      case "messengers":
        set(name, c.clean(() => messengers(c, name, v)));
        break;
      case "hours":
        set(name, c.clean(() => hours(c, name, v)));
        break;
      case "serviceArea":
        set(name, c.clean(() => serviceArea(c, name, v)));
        break;
      case "address":
        set(name, c.clean(() => address(c, name, v)));
        break;
      case "geo":
        set(name, c.clean(() => geo(c, name, v)));
        break;
      case "storefrontPhoto":
        set(name, c.clean(() => c.word(name, v, LIMIT.maxUrl, "https_url", s => httpsHost(s) !== null)));
        break;
      case "landmark":
        set(name, c.clean(() => landmark(c, name, v)));
        break;
      case "rating":
        set(name, c.clean(() => rating(c, name, v)));
        break;
      case "booking": {
        const { problems, config } = checkBookingConfig(v);
        for (const p of problems) c.fail(p.path === "" ? name : at(name, p.path), p.code);
        set(name, config);
        break;
      }
      case "reviewUrl":
        set(name, c.clean(() => c.word(name, v, LIMIT.maxUrl, "review_url", s => LIMIT.reviewHosts.some(h => h === httpsHost(s)))));
        break;
      default:
        c.fail(name, "unknown_field");
    }
  }
  return { live, problems: c.problems };
}
