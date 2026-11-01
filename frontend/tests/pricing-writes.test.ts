import { readFileSync } from "node:fs";

import { afterEach, describe, expect, it, vi } from "vitest";

import { type PricingModel, clearPricing, previewPrice } from "@/entities/pricing";
import { trySave } from "@/features/edit-pricing/model/save";
import { answersFor, needFor } from "@/features/preview-price/model/answers";
import { type Timers, createRunner } from "@/features/preview-price/model/runner";
import { PREVIEW_DEBOUNCE_MS } from "@/features/preview-price/model/use-preview";

import { pricingRoute } from "../scripts/stub-pricing.ts";

const cleaning = JSON.parse(readFileSync(new URL("./fixtures/pricing/valid/cleaning.json", import.meta.url), "utf8")) as PricingModel;
const item = (updated_at: string | null, by: string | null = "someone@example.test") => ({ brand_id: "vifnet", locales: ["fr", "en"], model: cleaning, updated_at, updated_by: by });

/** Each call answers the next of `replies`, and is recorded. */
function answer(...replies: [number, unknown][]) {
  const calls: { url: string; init: RequestInit }[] = [];
  vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
    calls.push({ url, init });
    const [status, body] = replies[Math.min(calls.length - 1, replies.length - 1)] ?? [500, {}];
    return new Response(JSON.stringify(body), { status });
  });
  return calls;
}
const sent = (call: { init: RequestInit } | undefined): unknown => JSON.parse(String(call?.init.body));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("saving a model", () => {
  it("PUTs exactly the model and the updated_at the draft started from", async () => {
    const calls = answer([200, item("2026-10-03T12:00:00Z")]);
    expect(await trySave("vifnet", cleaning, "2026-10-01T09:00:00Z")).toMatchObject({ kind: "saved", item: { updated_at: "2026-10-03T12:00:00Z" } });
    expect(calls[0]?.url).toBe("/api/sites/pricing/vifnet");
    expect(calls[0]?.init.method).toBe("PUT");
    expect(sent(calls[0])).toStrictEqual({ model: cleaning, expected_updated_at: "2026-10-01T09:00:00Z" });
  });

  it("on 409 hands back the pricing as it now is, and overwriting sends its updated_at", async () => {
    const current = item("2026-10-03T12:30:00Z", "colleague@example.test");
    const calls = answer([409, { error: "stale", current }], [200, item("2026-10-03T12:31:00Z")]);
    const outcome = await trySave("vifnet", cleaning, "2026-10-01T09:00:00Z");
    expect(outcome).toEqual({ kind: "conflict", current });
    if (outcome.kind !== "conflict" || !outcome.current) throw new Error("no current");
    // "Overwrite with my draft": the same model, over what is current.
    expect((await trySave("vifnet", cleaning, outcome.current.updated_at)).kind).toBe("saved");
    expect(sent(calls[1])).toStrictEqual({ model: cleaning, expected_updated_at: "2026-10-03T12:30:00Z" });
  });

  it("reads the pricing when a 409 does not carry it", async () => {
    const calls = answer([409, { error: "stale" }], [200, item("2026-10-03T12:30:00Z")]);
    expect(await trySave("vifnet", cleaning, null)).toMatchObject({ kind: "conflict", current: { updated_at: "2026-10-03T12:30:00Z" } });
    expect(calls[1]?.init.method).toBe("GET");
  });

  it("files a 422 under its path", async () => {
    answer([422, { error: 'no "en" label', path: "inputs.bedrooms.options.studio.labels.en" }]);
    expect(await trySave("vifnet", cleaning, null)).toEqual({ kind: "invalid", path: "inputs.bedrooms.options.studio.labels.en", message: 'no "en" label' });
  });

  it("taking pricing off is guarded like a save", async () => {
    const calls = answer([200, { ...item("2026-10-03T13:00:00Z"), model: null }]);
    await clearPricing("vifnet", "2026-10-03T12:00:00Z");
    expect(calls[0]?.init.method).toBe("DELETE");
    expect(sent(calls[0])).toStrictEqual({ expected_updated_at: "2026-10-03T12:00:00Z" });
  });
});

describe("the preview", () => {
  it("asks the server, sending the draft's model, the need and the answers", async () => {
    const calls = answer([200, { cents: 8400 }]);
    expect(await previewPrice("vifnet", cleaning, "standard", { zone: "proche" })).toBe(8400);
    expect(calls[0]?.url).toBe("/api/sites/pricing/vifnet/preview");
    expect(sent(calls[0])).toStrictEqual({ model: cleaning, need: "standard", inputs: { zone: "proche" } });
  });

  it("reads null as no price", async () => {
    answer([200, { cents: null }]);
    expect(await previewPrice("vifnet", cleaning, "standard", {})).toBeNull();
  });

  it("answers every asked question, the picked option where the model still has it", () => {
    expect(answersFor(cleaning, "standard", { zone: "loin", bedrooms: "gone" })).toEqual({ zone: "loin", bedrooms: "studio", surface: "s40", frequency: "weekly" });
    expect(answersFor(cleaning, "windows", { zone: "loin" })).toEqual({});
    expect(needFor(cleaning, "gone")).toBe("standard");
    expect(needFor({ ...cleaning, needs: {} }, null)).toBeNull();
  });
});

function fakeTimers(): Timers {
  return { setTimeout: (fn, ms) => setTimeout(fn, ms), clearTimeout: (id) => clearTimeout(id as ReturnType<typeof setTimeout>) };
}

describe("the preview's debounce", () => {
  it("asks once typing pauses for 300 ms, with the latest draft", async () => {
    vi.useFakeTimers();
    const run = vi.fn(async (n: number) => n * 100);
    const settled: [string, unknown][] = [];
    const runner = createRunner({ delayMs: PREVIEW_DEBOUNCE_MS, run, onSettled: (key, r) => settled.push([key, r]), timers: fakeTimers() });
    runner.request("a", 1);
    await vi.advanceTimersByTimeAsync(200);
    runner.request("b", 2);
    await vi.advanceTimersByTimeAsync(299);
    expect(run).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(run).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledWith(2);
    expect(settled).toEqual([["b", { ok: true, value: 200 }]]);
  });

  it("drops a slow answer to an older draft", async () => {
    vi.useFakeTimers();
    const resolvers: ((v: number) => void)[] = [];
    const run = (n: number) => new Promise<number>((resolve) => resolvers.push(() => resolve(n)));
    const settled: string[] = [];
    const runner = createRunner({ delayMs: PREVIEW_DEBOUNCE_MS, run, onSettled: (key) => settled.push(key), timers: fakeTimers() });
    runner.request("old", 1);
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS);
    runner.request("new", 2);
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS);
    resolvers[1]?.(2);
    resolvers[0]?.(1);
    await vi.runAllTimersAsync();
    expect(settled).toEqual(["new"]);
  });

  it("asks nothing after a cancel, and works again after it", async () => {
    vi.useFakeTimers();
    const run = vi.fn(async () => 1);
    const runner = createRunner({ delayMs: PREVIEW_DEBOUNCE_MS, run, onSettled: () => undefined, timers: fakeTimers() });
    runner.request("a", 1);
    runner.cancel();
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS * 2);
    expect(run).not.toHaveBeenCalled();
    runner.request("b", 2);
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS);
    expect(run).toHaveBeenCalledTimes(1);
  });
});

describe("the dev stub", () => {
  const route = (method: string, path: string, body: Record<string, unknown> = {}, role = "admin") => pricingRoute(method, path, body, role, "t@example.test");

  it("lists vifnet with kitstart's model and aquafix with none", () => {
    const list = route("GET", "/pricing")?.body as { items: { brand_id: string; model: unknown }[] };
    expect(list.items.map((i) => [i.brand_id, i.model === null])).toEqual([
      ["aquafix", true],
      ["vifnet", false],
    ]);
  });

  it("refuses a save at a stale updated_at with the current pricing, and takes it at the current one", () => {
    const current = route("GET", "/pricing/vifnet")?.body as { updated_at: string };
    expect(route("PUT", "/pricing/vifnet", { model: cleaning, expected_updated_at: null })).toMatchObject({ status: 409, body: { error: "stale", current } });
    expect(route("PUT", "/pricing/vifnet", { model: cleaning, expected_updated_at: current.updated_at })).toMatchObject({ status: 200, changed: "vifnet" });
  });

  it("answers 422 with a path the editor can show", () => {
    const model = { ...cleaning, inputs: cleaning.inputs.map((i) => (i.id === "bedrooms" ? { ...i, labels: { fr: "Chambres" } } : i)) };
    expect(route("PUT", "/pricing/aquafix", { model, expected_updated_at: null })).toEqual({ status: 422, body: { path: "inputs.bedrooms.labels.en", error: 'no "en" label' } });
  });

  it("lets an operator read and preview, not write", () => {
    expect(route("PUT", "/pricing/aquafix", { model: cleaning, expected_updated_at: null }, "operator")?.status).toBe(403);
    expect(route("POST", "/pricing/aquafix/preview", { model: cleaning, need: "windows", inputs: {} }, "operator")).toMatchObject({ status: 200, body: { cents: 8900 } });
    expect(route("POST", "/pricing/aquafix/preview", { model: cleaning, need: "standard", inputs: {} }, "operator")).toMatchObject({ status: 200, body: { cents: null } });
  });
});
