import { describe, expect, it } from "vitest";

import { savedSince } from "@/entities/place/lib/live";
import { newLeads, parseSeen, seenAfter, unseen } from "@/views/nav/model/unseen";

describe("the nav's badges", () => {
  it("count new leads as the growth of the total since the screen was seen", () => {
    expect(newLeads(12, 9)).toBe(3);
    expect(newLeads(9, 9)).toBe(0);
    expect(newLeads(null, 9)).toBe(0);
    expect(newLeads(12, null)).toBe(0);
    expect(newLeads(5, 9)).toBe(0);
  });

  /** The marks a screen shows over a run of answers, each `[times its items changed at, whether the screen is open]`. */
  const marks = (answers: [number[], boolean][]) => {
    let seen: number | null = null;
    return answers.map(([times, open]) => {
      seen = seenAfter(seen, times, open);
      return unseen(times, seen);
    });
  };

  it("take the first answer as seen, and count what changed after it", () => {
    expect(marks([[[1, 5], false], [[1, 5, 7], false], [[1, 8, 9], false]])).toEqual([0, 1, 2]);
  });

  it("clear when their screen opens, and count from there", () => {
    expect(marks([[[1], false], [[1, 4], false], [[1, 4], true], [[1, 4, 6], false]])).toEqual([0, 1, 0, 1]);
  });

  it("do not mark the screen that is open, and count from an empty list too", () => {
    expect(marks([[[], false], [[3], true], [[3, 4], false]])).toEqual([0, 0, 1]);
    expect(marks([[[], false], [[3], false]])).toEqual([0, 1]);
  });

  it("keep the leads baseline only when storage holds a count", () => {
    expect(parseSeen("42|*", "*")).toBe(42);
    expect(parseSeen(null, "*")).toBeNull();
    expect(parseSeen("-1|*", "*")).toBeNull();
    expect(parseSeen("1e3|*", "*")).toBeNull();
    expect(parseSeen("{}", "*")).toBeNull();
  });

  it("drop the leads baseline counted under another selection", () => {
    expect(parseSeen("42|3,7", "3,7")).toBe(42);
    expect(parseSeen("42|3,7", "3")).toBeNull();
    expect(parseSeen("42|*", "")).toBeNull();
    expect(parseSeen("42", "*")).toBeNull();
  });
});

describe("a place's live changes", () => {
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
