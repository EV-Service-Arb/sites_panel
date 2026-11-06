/**
 * Bookings for the dev stub (docs/ARCHITECTURE.md, "Booking"): a lead's
 * booking in every status, the operator's writes with the backend's
 * transitions (409 from where they do not apply), Idempotency-Key replays,
 * the providers' bookings no lead was found for, and attaching one.
 *
 *   STUB_BOOKING_CONFLICT=1   every booking write answers 409, as if the provider moved first
 */
import { randomUUID } from "node:crypto";

type Json = Record<string, unknown>;

type Told = [string, string | null, string | null][];

export interface StubReply {
  status: number;
  body?: unknown;
  /** Live topics to announce once the reply is out: `[topic, brand, id]`. */
  told?: Told;
}

interface LeadBooking {
  status: "none" | "requested" | "booked" | "canceled" | "done" | "no_show";
  provider: string | null;
  start_at: string | null;
  end_at: string | null;
  external_ref: string | null;
  match: "ref" | "contact" | "manual" | null;
  preferred_date: string | null;
  preferred_part: "morning" | "afternoon" | "evening" | null;
}

interface ProviderBooking {
  id: string;
  brand: string;
  provider: string;
  external_ref: string;
  status: "booked" | "canceled";
  start_at: string;
  end_at: string | null;
  booked_at: string | null;
  last_event_at: string;
  lead: string | null;
  /** An operator attached it: attaching again is a 409. */
  byHand: boolean;
  contact: { name?: string; email?: string; phone?: string } | null;
}

const ALWAYS_CONFLICT = process.env.STUB_BOOKING_CONFLICT === "1";
const NONE: LeadBooking = { status: "none", provider: null, start_at: null, end_at: null, external_ref: null, match: null, preferred_date: null, preferred_part: null };

/** A whole hour `days` from today at `hour` local, as the backend writes instants (UTC, `Z`). */
function at(days: number, hour: number, minutes = 0): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  d.setHours(hour, minutes, 0, 0);
  return d.toISOString().replace(".000Z", "Z");
}
const day = (days: number) => at(days, 12).slice(0, 10);

/** Bookings by `brand/lead`; a lead missing here has none. */
const byLead = new Map<string, LeadBooking>([
  ["aquafix/stub-1", { ...NONE, status: "requested", provider: "manual", preferred_date: day(1), preferred_part: "morning" }],
  ["aquafix/stub-2", { ...NONE, status: "booked", provider: "google_calendar", start_at: at(1, 10), end_at: at(1, 11), external_ref: "gcal-7f3a", match: "contact" }],
  ["aquafix/stub-3", { ...NONE, status: "booked", provider: "manual", start_at: at(2, 14, 30), end_at: null }],
  ["aquafix/stub-4", { ...NONE, status: "canceled", provider: "cal_com", start_at: at(3, 9), end_at: at(3, 9, 30), external_ref: "cal-1182", match: "ref" }],
  ["aquafix/stub-5", { ...NONE, status: "done", provider: "manual", start_at: at(-1, 16), end_at: at(-1, 17), match: null }],
  ["aquafix/stub-8", { ...NONE, status: "no_show", provider: "google_calendar", start_at: at(-2, 11), end_at: at(-2, 12), external_ref: "gcal-11c0", match: "ref" }],
  ["vifnet/stub-7", { ...NONE, status: "requested", provider: "cal_com" }],
]);

const providerBookings: ProviderBooking[] = [
  {
    id: randomUUID(), brand: "aquafix", provider: "google_calendar", external_ref: "gcal-9b21", status: "booked", start_at: at(1, 15), end_at: at(1, 16),
    booked_at: at(0, 8), last_event_at: at(0, 8), lead: null, byHand: false,
    // Léa's number written another way: the picker puts her lead first.
    contact: { name: "L. Martin (stub)", phone: "06 00 00 00 03" },
  },
  {
    id: randomUUID(), brand: "vifnet", provider: "cal_com", external_ref: "cal-2210", status: "booked", start_at: at(4, 9), end_at: at(4, 10, 30),
    booked_at: at(0, 7), last_event_at: at(0, 7), lead: null, byHand: false, contact: { email: "client@example.test" },
  },
  {
    id: randomUUID(), brand: "aquafix", provider: "google_calendar", external_ref: "gcal-c004", status: "canceled", start_at: at(2, 8), end_at: null,
    booked_at: at(-1, 18), last_event_at: at(0, 6), lead: null, byHand: false, contact: null,
  },
];

/** `Lead.booking` as the API gives it. */
export function bookingDto(brand: string, lead: string): LeadBooking {
  return byLead.get(`${brand}/${lead}`) ?? NONE;
}


/** Replies already given, by Idempotency-Key: a retry gets the first answer back as 200. */
const replayed = new Map<string, StubReply>();

function once(key: string | undefined, write: () => StubReply): StubReply {
  const seen = key ? replayed.get(key) : undefined;
  if (seen) return { ...seen, status: seen.status === 201 ? 200 : seen.status, told: [] };
  const reply = write();
  if (key && reply.status < 300) replayed.set(key, reply);
  return reply;
}

const CLOSED = ["done", "no_show", "canceled"] as const;
const created = (told: Told): StubReply => ({ status: 201, body: { event_id: randomUUID() }, told });
const conflict = (why: string): StubReply => ({ status: 409, body: { error: why } });
const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:\d{2})$/;

function setSlot(key: string, b: LeadBooking, body: Json): StubReply {
  const start = typeof body.start_at === "string" && RFC3339.test(body.start_at) ? body.start_at : null;
  const end = body.end_at === undefined ? null : typeof body.end_at === "string" && RFC3339.test(body.end_at) ? body.end_at : false;
  if (start === null || end === false) return { status: 400, body: { error: "start_at and end_at are RFC 3339 instants with an offset" } };
  if (end !== null && Date.parse(end) <= Date.parse(start)) return { status: 400, body: { error: "end_at is not after start_at" } };
  if (b.status === "done") return conflict("the booking took place; it cannot be set again");
  const iso = (s: string) => new Date(s).toISOString().replace(".000Z", "Z");
  byLead.set(key, { ...b, status: "booked", provider: b.provider ?? "manual", start_at: iso(start), end_at: end && iso(end) });
  return created([]);
}

/** `POST /leads/{brand}/{lead}/booking[/status]`: `rest` is `/booking` or `/booking/status`. */
export function bookingWrite(brand: string, lead: string, rest: string, body: Json, idempotencyKey: string | undefined): StubReply {
  const key = `${brand}/${lead}`;
  return once(idempotencyKey && `${key}${rest}:${idempotencyKey}`, () => {
    if (ALWAYS_CONFLICT) return conflict("the booking changed meanwhile");
    const b = bookingDto(brand, lead);
    if (rest === "/booking" && body.action === "set") return setSlot(key, b, body);
    if (rest === "/booking" && body.action === "clear") {
      if (b.status === "none" || b.status === "done") return conflict(`nothing to clear from ${b.status}`);
      byLead.set(key, NONE);
      return created([]);
    }
    const closed = CLOSED.find((s) => s === body.status);
    if (rest === "/booking/status" && closed) {
      if (b.status !== "booked") return conflict(`only a booked slot closes; this one is ${b.status}`);
      byLead.set(key, { ...b, status: closed });
      return created([]);
    }
    return { status: 400, body: { error: "not a booking action" } };
  });
}

/** `GET /bookings/unmatched` and `POST /bookings/{id}/attach`; null for any other path. */
export function bookingsRoute(method: string, path: string, query: URLSearchParams, body: Json, leadExists: (brand: string, lead: string) => boolean): StubReply | null {
  if (path === "/bookings/unmatched" && method === "GET") {
    const brand = query.get("brand");
    const list = providerBookings
      .filter((b) => b.lead === null && (!brand || b.brand === brand))
      .sort((a, b) => a.start_at.localeCompare(b.start_at))
      .map((b) => ({
        id: b.id, brand: b.brand, provider: b.provider, external_ref: b.external_ref, status: b.status, start_at: b.start_at, end_at: b.end_at,
        booked_at: b.booked_at, last_event_at: b.last_event_at, lead_id: null, match: null, ...(b.contact ? { contact: b.contact } : {}),
      }));
    return { status: 200, body: { bookings: list } };
  }
  const m = /^\/bookings\/([^/]+)\/attach$/.exec(path);
  if (!m || method !== "POST") return null;
  const booking = providerBookings.find((b) => b.id === decodeURIComponent(m[1] ?? ""));
  const lead = String(body.lead ?? "");
  if (!booking || !leadExists(booking.brand, lead)) return { status: 404, body: { error: "not_found" } };
  if (booking.byHand && booking.lead === lead) return conflict("attached to that lead by hand already");
  const left = booking.lead;
  booking.lead = lead;
  booking.byHand = true;
  byLead.set(`${booking.brand}/${lead}`, {
    ...bookingDto(booking.brand, lead),
    status: booking.status, provider: booking.provider, start_at: booking.start_at, end_at: booking.end_at, external_ref: booking.external_ref, match: "manual",
  });
  const told: Told = [["bookings", booking.brand, null], ["lead", booking.brand, lead]];
  if (left) told.push(["lead", booking.brand, left]);
  return created(told);
}

/** Someone books at a provider and no lead matches: a new row under "without a lead". */
export function arriveUnmatched(): string {
  const brand = Math.random() < 0.7 ? "aquafix" : "vifnet";
  const now = new Date().toISOString().replace(/\.\d+Z$/, "Z");
  providerBookings.push({
    id: randomUUID(), brand, provider: brand === "aquafix" ? "google_calendar" : "cal_com", external_ref: `live-${randomUUID().slice(0, 6)}`, status: "booked",
    start_at: at(1 + Math.floor(Math.random() * 5), 9 + Math.floor(Math.random() * 8)), end_at: null, booked_at: now, last_event_at: now, lead: null, byHand: false,
    contact: { name: "Walk-in booking (stub)", phone: "+33 6 00 00 00 77" },
  });
  return brand;
}

/** The provider moves a booked slot an hour later, as a calendar sync would report it. */
export function providerMoves(): [string, string] | null {
  const entry = [...byLead.entries()].find(([, b]) => b.status === "booked" && b.provider !== "manual" && b.start_at);
  if (!entry) return null;
  const [key, b] = entry;
  const shift = (s: string | null) => (s === null ? null : new Date(Date.parse(s) + 3_600_000).toISOString().replace(".000Z", "Z"));
  byLead.set(key, { ...b, start_at: shift(b.start_at), end_at: shift(b.end_at) });
  const [brand = "", lead = ""] = key.split("/");
  return [brand, lead];
}
