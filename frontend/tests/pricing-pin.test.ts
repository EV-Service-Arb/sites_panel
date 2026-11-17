import { readFileSync } from "node:fs";

import type { PricingModel } from "@evinvest/sites-contract";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type PricingItem, pricingSavedSince } from "@/entities/pricing";
import { tryClear } from "@/features/clear-pricing/model/clear";
import { pinFrom } from "@/views/pricing/model/pin";

const cleaning = JSON.parse(readFileSync(new URL("../../contracts/vectors/pricing/valid/cleaning.json", import.meta.url), "utf8")) as PricingModel;
const item = (updated_at: string, model: PricingModel | null = cleaning, by = "admin@example.test"): PricingItem => ({
  brand_id: "vifnet",
  locales: ["fr", "en"],
  model,
  updated_at,
  updated_by: by,
});
const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

/**
 * The screen's loop around a write, as BrandPricing runs it: `written(item?)`
 * pins `item` (or nothing) and reloads the read, whose last answer stays on
 * screen, not fresh, until the next lands; a render pins from the read only through `pinFrom`.
 */
function screen(first: PricingItem) {
  let pinned: PricingItem | null = null;
  let shown = { latest: first, fresh: true };
  const render = () => {
    const { latest, fresh } = shown;
    pinned = pinFrom(pinned, latest, fresh) ?? pinned;
    const someoneElse = latest !== null && pinned !== null && pricingSavedSince(latest, pinned);
    return { pinned, latest, someoneElse };
  };
  return {
    render,
    written(next?: PricingItem) {
      pinned = next ?? null;
      shown = { ...shown, fresh: false };
      return render();
    },
    land(read: PricingItem) {
      shown = { latest: read, fresh: true };
      return render();
    },
  };
}

afterEach(() => vi.unstubAllGlobals());

describe("one's own write is never someone else's", () => {
  it("taking the pricing off pins what the DELETE answered", async () => {
    const before = item("2026-10-03T12:00:00Z");
    const removed = item("2026-10-03T13:00:00Z", null);
    vi.stubGlobal("fetch", async () => json(200, removed));
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

  it("'Load latest' after a stale write loads the fresh pricing at once", () => {
    const mine = item("2026-10-03T12:00:00Z");
    const theirs = item("2026-10-03T12:30:00Z", cleaning, "colleague@example.test");
    const s = screen(mine);
    s.render();
    s.written();
    expect(s.land(theirs)).toMatchObject({ pinned: theirs, someoneElse: false });
  });
});

describe("taking the pricing off when someone wrote first", () => {
  it("reads the brand again and hands back what they wrote, so it is shown rather than removed unseen", async () => {
    const theirs = item("2026-10-03T12:30:00Z", cleaning, "colleague@example.test");
    const asked: string[] = [];
    vi.stubGlobal("fetch", async (request: Request) => {
      asked.push(`${request.method} ${new URL(request.url).pathname}`);
      return request.method === "DELETE" ? json(409, { code: "stale", error: "someone saved first" }) : json(200, theirs);
    });
    expect(await tryClear(item("2026-10-03T12:00:00Z"))).toEqual({ kind: "conflict", current: theirs });
    expect(asked).toEqual(["DELETE /api/sites/pricing/vifnet", "GET /api/sites/pricing/vifnet"]);
  });
});
