import { describe, expect, it } from "vitest";

import { bookingWrite, bookingsRoute } from "../scripts/stub-bookings.ts";
import { unmatchedListParser } from "@/entities/booking/model/booking";
import { bookingWhen } from "@/entities/lead/lib/booking-text";
import { NO_BOOKING, bookingActions } from "@/entities/lead/model/booking";
import { type Lead, leadParser } from "@/entities/lead/model/lead";
import { rankCandidates, sameContact, typedId } from "@/features/attach-booking/model/candidates";
import { leadFilterFrom, narrows, paramsWith } from "@/features/lead-filters/model/params";
import { bookingRefusal } from "@/features/manage-booking/model/failure";
import { slotBody, slotDraftOf, slotProblems } from "@/features/manage-booking/model/slot-draft";
import { attemptFor } from "@/shared/api/idempotency";
import { createHttp } from "@/shared/api/http";
import { translator } from "@/shared/i18n/translate";
import { formatSlot, localInstant, rfc3339 } from "@/shared/lib/instant";
import { parse } from "@/shared/lib/parse";

const en = translator("en");
const row = { brand: "aquafix", lead_id: "lead-42-0a1b2c3d", stage: "created", manual: false, last_event_at: "2026-10-04T08:00:00Z" };
const booked = { status: "booked", provider: "google_calendar", start_at: "2026-10-06T08:00:00Z", end_at: "2026-10-06T09:00:00Z", external_ref: "ev1", match: "contact", preferred_date: null, preferred_part: null };

describe("a lead's booking as the API sends it", () => {
  it("reads every field", () => {
    expect(parse(leadParser, { ...row, booking: booked }).booking).toEqual(booked);
  });

  it("is none on a lead read from before bookings", () => {
    expect(parse(leadParser, row).booking).toEqual(NO_BOOKING);
  });

  it("refuses a status, a provider or a part of the day it does not know", () => {
    expect(() => parse(leadParser, { ...row, booking: { ...booked, status: "pending" } })).toThrow(/booking\.status/);
    expect(() => parse(leadParser, { ...row, booking: { ...booked, provider: "calendly" } })).toThrow(/booking\.provider/);
    expect(() => parse(leadParser, { ...row, booking: { ...NO_BOOKING, preferred_part: "night" } })).toThrow(/preferred_part/);
  });
});

describe("the bookings without a lead", () => {
  const base = { id: "b1", brand: "aquafix", provider: "google_calendar", external_ref: "e1", status: "booked", start_at: "2026-10-06T08:00:00Z", end_at: null, booked_at: null, last_event_at: "2026-10-04T08:00:00Z", lead_id: null, match: null };

  it("read with the attendee's contact, or without it for a role that does not see it", () => {
    const list = parse(unmatchedListParser, { bookings: [{ ...base, contact: { phone: "+33600000003" } }, { ...base, id: "b2", status: "canceled" }] });
    expect(list.bookings[0]?.contact).toEqual({ name: null, email: null, phone: "+33600000003" });
    expect(list.bookings[1]?.contact).toBeNull();
    expect(() => parse(unmatchedListParser, { bookings: [{ ...base, status: "done" }] })).toThrow(/status/);
  });
});

describe("the operator's actions", () => {
  // `BookingStatus::allows`, the backend's transitions test row by row.
  it.each([
    ["none", ["set"]],
    ["requested", ["set", "clear"]],
    ["booked", ["set", "done", "no_show", "canceled", "clear"]],
    ["canceled", ["set", "clear"]],
    ["done", []],
    ["no_show", ["set", "clear"]],
  ] as const)("from %s offer %j", (status, actions) => {
    expect(bookingActions(status)).toEqual(actions);
  });
});

describe("a slot as sent", () => {
  it("is RFC 3339 with the zone's offset, to the second", () => {
    const at = new Date(Date.UTC(2026, 9, 6, 8, 0, 0));
    expect(rfc3339(at, 120)).toBe("2026-10-06T10:00:00+02:00");
    expect(rfc3339(at, 0)).toBe("2026-10-06T08:00:00+00:00");
    expect(rfc3339(at, -300)).toBe("2026-10-06T03:00:00-05:00");
    expect(rfc3339(at, 330)).toBe("2026-10-06T13:30:00+05:30");
  });

  it("keeps the wall-clock time typed, in the browser's zone, with the offset of each instant", () => {
    const body = slotBody({ day: "2026-10-06", start: "10:00", end: "11:30" });
    expect(body).toMatchObject({ action: "set" });
    if (body?.action !== "set") throw new Error("a valid draft makes a body");
    expect(body.start_at).toMatch(/^2026-10-06T10:00:00[+-]\d{2}:\d{2}$/);
    expect(body.end_at).toMatch(/^2026-10-06T11:30:00[+-]\d{2}:\d{2}$/);
    expect(Date.parse(body.start_at)).toBe(localInstant("2026-10-06", "10:00")?.getTime());
  });

  it("asks each instant its own offset, so a start before the autumn change and an end after it differ", () => {
    const change = localInstant("2026-10-25", "10:30")?.getTime() ?? 0;
    const body = slotBody({ day: "2026-10-25", start: "10:00", end: "11:00" }, (d) => (d.getTime() < change ? 120 : 60));
    if (body?.action !== "set") throw new Error("a valid draft makes a body");
    expect(body.start_at).toMatch(/\+02:00$/);
    expect(body.end_at).toMatch(/\+01:00$/);
  });

  it("leaves the end out when none is typed, and takes times as people type them", () => {
    expect(slotBody({ day: "2026-10-06", start: "9h30", end: " " })).toEqual({ action: "set", start_at: expect.stringMatching(/^2026-10-06T09:30:00/) });
  });

  it("is refused before sending: no day, a bad time, an end not after the start", () => {
    expect(slotProblems({ day: "", start: "25:00", end: "x" })).toEqual(["day", "start", "end"]);
    expect(slotProblems({ day: "2026-02-30", start: "10:00", end: "" })).toEqual(["day"]);
    expect(slotProblems({ day: "2026-10-06", start: "10:00", end: "10:00" })).toEqual(["order"]);
    expect(slotBody({ day: "2026-10-06", start: "10:00", end: "09:00" })).toBeNull();
  });

  it("starts from the visitor's wish when there is no slot yet", () => {
    expect(slotDraftOf({ ...NO_BOOKING, status: "requested", preferred_date: "2026-10-07", preferred_part: "afternoon" })).toEqual({ day: "2026-10-07", start: "14:00", end: "" });
    expect(slotDraftOf(NO_BOOKING)).toEqual({ day: "", start: "", end: "" });
  });
});

describe("a slot as read", () => {
  it("names its zone", () => {
    const text = formatSlot("2026-10-06T08:00:00Z", "2026-10-06T09:00:00Z", "en-GB", "Europe/Paris");
    expect(text).toMatch(/10:00/);
    expect(text).toMatch(/11:00/);
    expect(text).toMatch(/CEST|GMT\+2/);
  });

  it("says the wish of a booking only asked for", () => {
    const asked = { ...NO_BOOKING, status: "requested" as const, preferred_date: "2026-10-07", preferred_part: "morning" as const };
    expect(bookingWhen(asked, en, "en-GB")).toBe("asked for 7 Oct 2026, morning");
    expect(bookingWhen({ ...asked, preferred_date: null, preferred_part: null }, en, "en-GB")).toBeNull();
    expect(bookingWhen(NO_BOOKING, en, "en-GB")).toBeNull();
  });
});

describe("a refused booking write", () => {
  const failing = (status: number, body: unknown) =>
    createHttp({ fetch: async () => new Response(JSON.stringify(body), { status }), cookie: () => "", host: () => ({ api: "", csrfCookie: "sa_csrf" }), onUnauthenticated: () => {} })
      .send("POST", "/x", {}, () => null)
      .catch((e: unknown) => e);

  it("409: says the booking moved on and reads the card again", async () => {
    expect(bookingRefusal(await failing(409, { error: "not from booked" }))).toEqual({ key: "booking.conflict", reread: true });
  });

  it("404 and 400 have their own words; anything else is the general failure", async () => {
    expect(bookingRefusal(await failing(404, { error: "not_found" }))).toMatchObject({ key: "booking.notFound", reread: true });
    expect(bookingRefusal(await failing(400, { error: "start_at is not an instant" }))).toEqual({ key: "booking.badTime", detail: "start_at is not an instant", reread: false });
    expect(bookingRefusal(await failing(500, {}))).toBeNull();
  });
});

describe("the Idempotency-Key", () => {
  it("rides on the write beside the CSRF token", async () => {
    const sent: Headers[] = [];
    const http = createHttp({
      fetch: async (_input, init) => {
        sent.push(new Headers(init?.headers));
        return new Response(JSON.stringify({ event_id: "e" }), { status: 201 });
      },
      cookie: () => "sa_csrf=c",
      host: () => ({ api: "", csrfCookie: "sa_csrf" }),
      onUnauthenticated: () => {},
    });
    await http.send("POST", "/x", { action: "clear" }, () => null, { "idempotency-key": "k1" });
    expect(sent.map((h) => [h.get("idempotency-key"), h.get("x-sa-csrf")])).toEqual([["k1", "c"]]);
  });

  it("stays with a retried body and changes with a new one", () => {
    let n = 0;
    const mint = () => `k${++n}`;
    const first = attemptFor(null, { status: "done" }, mint);
    expect(attemptFor(first, { status: "done" }, mint)).toBe(first);
    expect(attemptFor(first, { status: "no_show" }, mint).key).toBe("k2");
  });
});

describe("the booking filter in the URL", () => {
  it("takes a status the API takes and nothing else", () => {
    expect(leadFilterFrom(new URLSearchParams("booking=booked")).booking).toBe("booked");
    expect(leadFilterFrom(new URLSearchParams("booking=none")).booking).toBe("none");
    expect(leadFilterFrom(new URLSearchParams("booking=pending")).booking).toBeNull();
  });

  it("goes into the URL and back, and narrows the list", () => {
    const on = paramsWith(new URLSearchParams("stage=created"), { booking: "no_show" });
    expect(on.toString()).toBe("stage=created&booking=no_show");
    expect(narrows(leadFilterFrom(new URLSearchParams("booking=requested")))).toBe(true);
    expect(paramsWith(on, { booking: null }).toString()).toBe("stage=created");
  });
});

describe("picking the lead to attach", () => {
  const lead = (id: string, pii: Record<string, unknown>): Lead => parse(leadParser, { ...row, lead_id: id, pii });

  it("knows the contact however it was written", () => {
    expect(sameContact(lead("a", { phone: "+33 6 00 00 00 03" }), { name: null, email: null, phone: "06 00 00 00 03" })).toBe(true);
    expect(sameContact(lead("a", { email: "Client@Example.test" }), { name: null, email: "client@example.test ", phone: null })).toBe(true);
    expect(sameContact(lead("a", { phone: "+33 6 00 00 00 04" }), { name: null, email: null, phone: "06 00 00 00 03" })).toBe(false);
    expect(sameContact(lead("a", { phone: "03" }), { name: null, email: null, phone: "03" })).toBe(false);
  });

  it("puts the leads with the booking's contact first, the rest as listed", () => {
    const ranked = rankCandidates([lead("a", {}), lead("b", { phone: "0600000003" }), lead("c", {})], { name: null, email: null, phone: "+33600000003" });
    expect(ranked.map((r) => [r.lead.lead_id, r.same])).toEqual([["b", true], ["a", false], ["c", false]]);
  });

  it("offers a typed id only when it is one word", () => {
    expect(typedId(" lead-42-0a1b2c3d ")).toBe("lead-42-0a1b2c3d");
    expect(typedId("Camille Martin")).toBeNull();
    expect(typedId("  ")).toBeNull();
  });
});

describe("the dev stub's bookings", () => {
  it("refuses a close from anywhere but booked, and replays a retry under its key", () => {
    expect(bookingWrite("aquafix", "stub-1", "/booking/status", { status: "done" }, undefined).status).toBe(409);
    const first = bookingWrite("aquafix", "stub-3", "/booking/status", { status: "done" }, "k-done");
    expect(first.status).toBe(201);
    expect(bookingWrite("aquafix", "stub-3", "/booking/status", { status: "done" }, "k-done").status).toBe(200);
    expect(bookingWrite("aquafix", "stub-3", "/booking/status", { status: "done" }, "k-other").status).toBe(409);
    expect(bookingWrite("aquafix", "stub-3", "/booking", { action: "set", start_at: "2026-10-06T10:00:00+02:00" }, undefined).status).toBe(409);
  });

  it("refuses an instant without an offset", () => {
    expect(bookingWrite("aquafix", "stub-9", "/booking", { action: "set", start_at: "2026-10-06T10:00" }, undefined).status).toBe(400);
    expect(bookingWrite("aquafix", "stub-9", "/booking", { action: "set", start_at: "2026-10-06T10:00:00+02:00" }, undefined).status).toBe(201);
  });

  it("attaches a booking once: 404 for a lead the brand lacks, 409 for the same again", () => {
    const list = bookingsRoute("GET", "/bookings/unmatched", new URLSearchParams("brand=aquafix"), {}, () => true)?.body as { bookings: { id: string }[] };
    const id = list.bookings[0]?.id ?? "";
    const exists = (brand: string, lead: string) => brand === "aquafix" && lead === "stub-3";
    expect(bookingsRoute("POST", `/bookings/${id}/attach`, new URLSearchParams(), { lead: "nope" }, exists)?.status).toBe(404);
    expect(bookingsRoute("POST", `/bookings/${id}/attach`, new URLSearchParams(), { lead: "stub-3" }, exists)?.status).toBe(201);
    expect(bookingsRoute("POST", `/bookings/${id}/attach`, new URLSearchParams(), { lead: "stub-3" }, exists)?.status).toBe(409);
  });
});
