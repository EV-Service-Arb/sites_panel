import { describe, expect, it } from "vitest";

import { bookingWhen } from "@/entities/lead/lib/booking-text";
import { rankCandidates, sameContact, typedId } from "@/features/attach-booking/model/candidates";
import { bookingRefusal } from "@/features/manage-booking/model/failure";
import { slotBody, slotDraftOf, slotProblems } from "@/features/manage-booking/model/slot-draft";
import { attemptFor } from "@/shared/api/idempotency";
import { translator } from "@/shared/i18n/translate";
import { formatSlot, localInstant, rfc3339 } from "@/shared/lib/instant";

import { NO_BOOKING, lead as leadOf } from "./fixtures";

const en = translator("en");

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
  it("a conflict says the booking moved on and reads the card again", () => {
    for (const code of ["booking_done", "no_booking", "not_booked"] as const) expect(bookingRefusal({ code, error: "x" })).toEqual({ key: "booking.conflict", reread: true });
  });

  it("not found and a bad instant have their own words; anything else is the general failure", () => {
    expect(bookingRefusal({ code: "not_found", error: "no such lead" })).toMatchObject({ key: "booking.notFound", reread: true });
    expect(bookingRefusal({ code: "bad_request", error: "start_at is not an instant" })).toEqual({ key: "booking.badTime", detail: "start_at is not an instant", reread: false });
    expect(bookingRefusal({ code: "internal", error: "internal error" })).toBeNull();
  });
});

describe("the Idempotency-Key", () => {
  it("stays with a retried body and changes with a new one", () => {
    let n = 0;
    const mint = () => `k${++n}`;
    const first = attemptFor(null, { status: "done" }, mint);
    expect(attemptFor(first, { status: "done" }, mint)).toBe(first);
    expect(attemptFor(first, { status: "no_show" }, mint).key).toBe("k2");
  });
});

describe("picking the lead to attach", () => {
  const lead = (id: string, pii: Record<string, unknown>) => leadOf({ lead_id: id, pii });

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
