import { describe, expect, it } from "vitest";

import { followsPlace, savedSince } from "@/entities/place/lib/live";
import type { ChangedEvent } from "@/shared/lib/live/protocol";
import { NOTHING_UNSEEN, newLeads, noteChanges, parseSeen, visit } from "@/views/nav/model/unseen";

const at = "2026-10-03T10:00:00Z";
const ev = (topic: ChangedEvent["topic"], brand_id: string | null = "aquafix", id: string | null = null): ChangedEvent => ({ topic, brand_id, id, at });

describe("the nav's badges", () => {
  it("count new leads as the growth of the total since the screen was seen", () => {
    expect(newLeads(12, 9)).toBe(3);
    expect(newLeads(9, 9)).toBe(0);
    expect(newLeads(null, 9)).toBe(0);
    expect(newLeads(12, null)).toBe(0);
    expect(newLeads(5, 9)).toBe(0);
  });

  it("count changed places once each, not once per save", () => {
    const s = noteChanges(NOTHING_UNSEEN, [ev("places", "aquafix", "lyon-3"), ev("places", "aquafix", "lyon-3"), ev("places", "vifnet", "paris-11")], "leads");
    expect(s.places).toHaveLength(2);
  });

  it("mark experiments with a dot, and nothing for the other topics", () => {
    const s = noteChanges(NOTHING_UNSEEN, [ev("experiments"), ev("pricing"), ev("invoices")], "overview");
    expect(s).toEqual({ places: [], experiments: true, bookings: false });
  });

  it("mark bookings without a lead with a dot on Leads, cleared by opening Leads", () => {
    const s = noteChanges(NOTHING_UNSEEN, [ev("bookings")], "overview");
    expect(s.bookings).toBe(true);
    expect(noteChanges(NOTHING_UNSEEN, [ev("bookings")], "leads")).toBe(NOTHING_UNSEEN);
    expect(visit(s, "leads").bookings).toBe(false);
    expect(visit(s, "places")).toBe(s);
  });

  it("do not mark the screen that is open", () => {
    expect(noteChanges(NOTHING_UNSEEN, [ev("places", "aquafix", "lyon-3")], "places")).toBe(NOTHING_UNSEEN);
    expect(noteChanges(NOTHING_UNSEEN, [ev("experiments")], "experiments")).toBe(NOTHING_UNSEEN);
  });

  it("clear when their screen opens, and only theirs", () => {
    const s = { places: ["aquafix/lyon-3"], experiments: true, bookings: false };
    expect(visit(s, "places")).toEqual({ places: [], experiments: true, bookings: false });
    expect(visit(s, "experiments").experiments).toBe(false);
    expect(visit(s, null)).toBe(s);
  });

  it("keep the leads baseline only when storage holds a count", () => {
    expect(parseSeen("42")).toBe(42);
    expect(parseSeen(null)).toBeNull();
    expect(parseSeen("-1")).toBeNull();
    expect(parseSeen("1e3")).toBeNull();
    expect(parseSeen("{}")).toBeNull();
  });
});

describe("a place's live changes", () => {
  const key = { brand: "aquafix", slug: "lyon-3" };

  it("follow that place, or any when the event names none", () => {
    const follows = followsPlace(key);
    expect(follows(ev("places", "aquafix", "lyon-3"))).toBe(true);
    expect(follows(ev("places", "aquafix", null))).toBe(true);
    expect(follows(ev("places", null, null))).toBe(true);
    expect(follows(ev("places", "aquafix", "lyon-7"))).toBe(false);
    expect(follows(ev("leads", "aquafix", "lyon-3"))).toBe(false);
  });

  it("count as someone else's only when strictly newer than what the form was read at", () => {
    const base = { updated_at: "2026-10-03T10:00:00Z" };
    expect(savedSince({ updated_at: "2026-10-03T10:01:00Z" }, base)).toBe(true);
    expect(savedSince(base, base)).toBe(false);
    // The older answer still on screen while one's own save is re-read.
    expect(savedSince({ updated_at: "2026-10-03T09:00:00Z" }, base)).toBe(false);
    expect(savedSince({ updated_at: "2026-10-03T10:01:00Z" }, { updated_at: null })).toBe(true);
    expect(savedSince({ updated_at: null }, { updated_at: null })).toBe(false);
  });
});
