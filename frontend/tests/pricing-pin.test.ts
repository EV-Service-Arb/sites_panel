import { readFileSync } from "node:fs";

import type { PricingModel } from "@evinvest/sites-contract";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type PricingItem, pricingSavedSince } from "@/entities/pricing";
import { tryClear } from "@/features/clear-pricing/model/clear";
import { type Resource, shownOf } from "@/shared/lib/use-resource";
import { pinFrom } from "@/views/pricing/model/pin";

const cleaning = JSON.parse(readFileSync(new URL("../../contracts/vectors/pricing/valid/cleaning.json", import.meta.url), "utf8")) as PricingModel;
const item = (updated_at: string, model: PricingModel | null = cleaning, by = "admin@example.test"): PricingItem => ({
  brand_id: "vifnet",
  locales: ["fr", "en"],
  model,
  updated_at,
  updated_by: by,
});
const ok = (data: PricingItem): Resource<PricingItem> => ({ status: "ok", data });
const GROUP = "pricing:vifnet";

/**
 * The screen's loop around a write, as BrandPricing runs it: `written(item?)`
 * pins `item` (or nothing) and moves the read to the next version; a render
 * pins from the read only through `pinFrom`.
 */
function screen(first: PricingItem) {
  let version = 0;
  let pinned: PricingItem | null = null;
  let answered = { key: `${GROUP}:0`, group: GROUP, tick: 0, value: ok(first) };
  const render = () => {
    const shown = shownOf(answered, `${GROUP}:${version}`, GROUP, 0);
    const latest = shown.value.status === "ok" ? shown.value.data : null;
    pinned = pinFrom(pinned, latest, shown.fresh) ?? pinned;
    const someoneElse = latest !== null && pinned !== null && pricingSavedSince(latest, pinned);
    return { pinned, latest, someoneElse };
  };
  return {
    render,
    written(next?: PricingItem) {
      pinned = next ?? null;
      version += 1;
      return render();
    },
    land(read: PricingItem) {
      answered = { key: `${GROUP}:${version}`, group: GROUP, tick: 0, value: ok(read) };
      return render();
    },
  };
}

afterEach(() => vi.unstubAllGlobals());

describe("a read moved to a newer version", () => {
  it("keeps the last answer on screen, marked not fresh, until its own lands", () => {
    const before = { key: `${GROUP}:0`, group: GROUP, tick: 0, value: ok(item("2026-10-03T12:00:00Z")) };
    expect(shownOf(before, `${GROUP}:1`, GROUP, 0)).toEqual({ value: before.value, fresh: false });
    expect(shownOf({ ...before, key: `${GROUP}:1` }, `${GROUP}:1`, GROUP, 0).fresh).toBe(true);
  });

  it("is loading, not another record's answer, outside its group", () => {
    const other = { key: "pricing:aquafix:0", group: "pricing:aquafix", tick: 0, value: ok(item("2026-10-03T12:00:00Z")) };
    expect(shownOf(other, `${GROUP}:0`, GROUP, 0)).toEqual({ value: { status: "loading" }, fresh: false });
  });
});

describe("one's own write is never someone else's", () => {
  it("taking the pricing off pins what the DELETE answered", async () => {
    const before = item("2026-10-03T12:00:00Z");
    const removed = item("2026-10-03T13:00:00Z", null);
    vi.stubGlobal("fetch", async () => new Response(JSON.stringify(removed), { status: 200 }));
    const outcome = await tryClear(before);
    expect(outcome).toEqual({ kind: "cleared", item: removed });
    if (outcome.kind !== "cleared") throw new Error("not cleared");

    const s = screen(before);
    s.render();
    expect(s.written(outcome.item)).toMatchObject({ pinned: removed, someoneElse: false });
    expect(s.land(removed)).toMatchObject({ pinned: removed, someoneElse: false });
  });

  it("re-reading after a write waits for the re-read instead of pinning the answer from before it", () => {
    const before = item("2026-10-03T12:00:00Z");
    const removed = item("2026-10-03T13:00:00Z", null);
    const s = screen(before);
    s.render();
    // The old answer is still on screen: nothing gets pinned from it.
    expect(s.written()).toMatchObject({ pinned: null, latest: before, someoneElse: false });
    expect(s.land(removed)).toMatchObject({ pinned: removed, someoneElse: false });
  });

  it("'Load latest' after a 409 that named nothing loads the fresh pricing at once", () => {
    const mine = item("2026-10-03T12:00:00Z");
    const theirs = item("2026-10-03T12:30:00Z", cleaning, "colleague@example.test");
    const s = screen(mine);
    s.render();
    s.written();
    expect(s.land(theirs)).toMatchObject({ pinned: theirs, someoneElse: false });
  });
});

describe("taking the pricing off when someone wrote first", () => {
  it("hands back what they wrote, so it is shown rather than removed unseen", async () => {
    const theirs = item("2026-10-03T12:30:00Z", cleaning, "colleague@example.test");
    vi.stubGlobal("fetch", async () => new Response(JSON.stringify({ error: "stale", current: theirs }), { status: 409 }));
    expect(await tryClear(item("2026-10-03T12:00:00Z"))).toEqual({ kind: "conflict", current: theirs });
  });

  it("has nothing to hand back when the 409 names nothing", async () => {
    vi.stubGlobal("fetch", async () => new Response(JSON.stringify({ error: "stale" }), { status: 409 }));
    expect(await tryClear(item("2026-10-03T12:00:00Z"))).toEqual({ kind: "conflict", current: null });
  });
});
