import { afterEach, describe, expect, it, vi } from "vitest";

import { addPlace, revertSettingsChange } from "@/entities/place/api/settings";
import { placeSettingsRoute } from "../scripts/stub-places.ts";

const view = { brand: "aquafix", slug: "royat", withdrawn: false, settings: {}, updated_at: null, updated_by: null, can_edit: true };

function answer(status: number, body: unknown) {
  const calls: { url: string; init: RequestInit }[] = [];
  vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
    calls.push({ url, init });
    return new Response(JSON.stringify(body), { status });
  });
  return calls;
}

afterEach(() => vi.unstubAllGlobals());

describe("a revert", () => {
  it("carries the updated_at it was read at", async () => {
    const calls = answer(200, view);
    await revertSettingsChange({ brand: "aquafix", slug: "royat" }, "c-1", "2026-10-03T12:00:00Z");
    expect(calls[0]?.url).toBe("/api/sites/places/aquafix/royat/settings/revert/c-1");
    expect(JSON.parse(String(calls[0]?.init.body))).toEqual({ expected_updated_at: "2026-10-03T12:00:00Z" });
  });

  it("sends null for a place never set, and a stale one is a conflict", async () => {
    const calls = answer(409, { error: "conflict" });
    await expect(revertSettingsChange({ brand: "aquafix", slug: "royat" }, "c-1", null)).rejects.toMatchObject({ failure: { kind: "conflict" } });
    expect(JSON.parse(String(calls[0]?.init.body))).toEqual({ expected_updated_at: null });
  });
});

describe("adding a place", () => {
  it("answers the place's settings", async () => {
    answer(201, view);
    expect((await addPlace({ brand: "aquafix", slug: "royat" })).slug).toBe("royat");
  });

  it("tells an existing place apart as a conflict", async () => {
    answer(409, { error: "exists" });
    await expect(addPlace({ brand: "aquafix", slug: "royat" })).rejects.toMatchObject({ failure: { kind: "conflict", message: "exists" } });
  });
});

describe("the dev stub", () => {
  it("refuses a revert at a stale updated_at and accepts it at the current one", () => {
    const route = (method: string, path: string, body: Record<string, unknown> = {}) => placeSettingsRoute(method, path, body, "admin", "t@example.test");
    const current = route("GET", "/places/aquafix/lyon-3/settings")?.body as { updated_at: string };
    const history = route("GET", "/places/aquafix/lyon-3/settings/history")?.body as { changes: { id: string }[] };
    const path = `/places/aquafix/lyon-3/settings/revert/${history.changes[0]?.id ?? ""}`;
    expect(route("POST", path, { expected_updated_at: null })?.status).toBe(409);
    expect(route("POST", path, { expected_updated_at: current.updated_at })?.status).toBe(200);
  });

  it("registers a place once, answering the GET shape, then 409 exists", () => {
    const add = () => placeSettingsRoute("POST", "/places", { brand: "aquafix", slug: "test-add" }, "admin", "t@example.test");
    expect(add()).toMatchObject({ status: 201, body: { brand: "aquafix", slug: "test-add", settings: {}, updated_at: null } });
    expect(add()).toEqual({ status: 409, body: { error: "exists" } });
  });
});
