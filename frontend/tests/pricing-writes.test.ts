import { readFileSync } from "node:fs";

import type { PricingModel } from "@evinvest/sites-contract";
import { afterEach, describe, expect, it, vi } from "vitest";

import { trySave } from "@/features/edit-pricing/model/save";
import { answersFor, needFor } from "@/features/preview-price/model/answers";
import { type Timers, createRunner } from "@/features/preview-price/model/runner";
import { PREVIEW_DEBOUNCE_MS } from "@/features/preview-price/model/use-preview";

const cleaning = JSON.parse(readFileSync(new URL("../../contracts/vectors/pricing/valid/cleaning.json", import.meta.url), "utf8")) as PricingModel;
const item = (updated_at: string | null, by: string | null = "someone@example.test") => ({ brand_id: "vifnet", locales: ["fr", "en"], model: cleaning, updated_at, updated_by: by });

/** Each call answers the next of `replies`, and is recorded. */
function answer(...replies: [number, unknown][]) {
  const calls: { method: string; path: string; body: unknown }[] = [];
  vi.stubGlobal("fetch", async (request: Request) => {
    calls.push({ method: request.method, path: new URL(request.url).pathname, body: request.method === "GET" ? undefined : await request.json() });
    const [status, body] = replies[Math.min(calls.length - 1, replies.length - 1)] ?? [500, {}];
    return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
  });
  return calls;
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("saving a model", () => {
  it("PUTs exactly the model and the updated_at the draft started from", async () => {
    const calls = answer([200, item("2026-10-03T12:00:00Z")]);
    expect(await trySave("vifnet", cleaning, "2026-10-01T09:00:00Z")).toMatchObject({ kind: "saved", item: { updated_at: "2026-10-03T12:00:00Z" } });
    expect(calls).toEqual([{ method: "PUT", path: "/api/sites/pricing/vifnet", body: { model: cleaning, expected_updated_at: "2026-10-01T09:00:00Z" } }]);
  });

  it("when stale, reads the brand again, and overwriting sends what it read", async () => {
    const current = item("2026-10-03T12:30:00Z", "colleague@example.test");
    const calls = answer([409, { code: "stale", error: "someone saved first" }], [200, current], [200, item("2026-10-03T12:31:00Z")]);
    const outcome = await trySave("vifnet", cleaning, "2026-10-01T09:00:00Z");
    expect(outcome).toEqual({ kind: "conflict", current });
    if (outcome.kind !== "conflict" || !outcome.current) throw new Error("no current");
    // "Overwrite with my draft": the same model, over what is current.
    expect((await trySave("vifnet", cleaning, outcome.current.updated_at)).kind).toBe("saved");
    expect(calls.map((c) => [c.method, c.body])).toEqual([
      ["PUT", { model: cleaning, expected_updated_at: "2026-10-01T09:00:00Z" }],
      ["GET", undefined],
      ["PUT", { model: cleaning, expected_updated_at: "2026-10-03T12:30:00Z" }],
    ]);
  });

  it("tells a refused model in the server's own words", async () => {
    answer([422, { code: "invalid_pricing", error: 'inputs[1].labels.en: no "en" label' }]);
    expect(await trySave("vifnet", cleaning, null)).toEqual({ kind: "invalid", message: 'inputs[1].labels.en: no "en" label' });
  });
});

describe("the preview", () => {
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
