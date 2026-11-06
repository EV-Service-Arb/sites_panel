/**
 * Brand pricing for the dev stub: `/api/v1/pricing` over two brands — vifnet
 * with the contract vectors' `valid/cleaning.json` model, aquafix with none
 * (its sites on their baked prices). In memory, with a journal for the history.
 * Checked and priced by `@evinvest/sites-contract`.
 *
 *   STUB_PRICING_CONFLICT=1        every save and removal answers 409, as if someone saved first
 *   STUB_PRICING_INVALID=<path>    every save and preview answers 422 at that path
 *                                  (e.g. `inputs.bedrooms.options.studio.labels.en`)
 */
import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";

import { checkPricingModel, priceOf } from "@evinvest/sites-contract";

import type { StubReply } from "./stub-places.ts";

type Json = Record<string, unknown>;

interface StubPricing {
  locales: string[];
  model: Json | null;
  updated_at: string | null;
  updated_by: string | null;
}

interface Change {
  id: string;
  at: string;
  by: string;
  model: Json | null;
}

const ALWAYS_CONFLICT = process.env.STUB_PRICING_CONFLICT === "1";
const FORCED_INVALID = process.env.STUB_PRICING_INVALID ?? null;

const cleaning = JSON.parse(readFileSync(new URL("../../contracts/vectors/pricing/valid/cleaning.json", import.meta.url), "utf8")) as Json;
const iso = () => new Date().toISOString();
const daysAgo = (n: number) => new Date(Date.now() - n * 86_400_000).toISOString();

const brands = new Map<string, StubPricing>([
  ["aquafix", { locales: ["fr", "en"], model: null, updated_at: null, updated_by: null }],
  ["vifnet", { locales: ["fr", "en"], model: cleaning, updated_at: daysAgo(2), updated_by: "admin@example.test (stub)" }],
]);

const journal = new Map<string, Change[]>([["vifnet", [{ id: randomUUID(), at: daysAgo(2), by: "admin@example.test (stub)", model: cleaning }]]]);

const item = (brand: string, p: StubPricing): Json => ({ brand_id: brand, locales: p.locales, model: p.model, updated_at: p.updated_at, updated_by: p.updated_by });

const isObj = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);

/** The first thing wrong, as the server answers its 422; `locales` only on a save, as the server checks them. */
function firstProblem(m: unknown, locales: readonly string[]): { error: string; code: string; path: string } | null {
  if (FORCED_INVALID) return { error: "refused by STUB_PRICING_INVALID (stub)", code: "object", path: FORCED_INVALID };
  const first = checkPricingModel(m, locales).problems[0];
  return first ? { error: `${first.code} (stub)`, code: first.code, path: first.path } : null;
}

function write(brand: string, p: StubPricing, by: string, model: Json | null): void {
  p.model = model;
  p.updated_at = iso();
  p.updated_by = by;
  journal.set(brand, [{ id: randomUUID(), at: p.updated_at, by, model }, ...(journal.get(brand) ?? [])]);
}

/** Someone else saves vifnet (the live stub's doing): its minimum flips between 49 € and 59 €. */
export function touchPricing(by: string): void {
  const p = brands.get("vifnet");
  if (!p?.model) return;
  write("vifnet", p, by, { ...p.model, minimumCents: p.model.minimumCents === 4900 ? 5900 : 4900 });
}

/**
 * The pricing routes, or null when `path` (after `/api/v1`) is not one of
 * them. The caller has passed the gate and the CSRF check already. `changed`
 * names the brand a write changed, for the live socket.
 */
export function pricingRoute(method: string, path: string, body: Json, role: string, me: string): (StubReply & { changed?: string }) | null {
  if (path === "/pricing" && method === "GET") return { status: 200, body: { items: [...brands].map(([b, p]) => item(b, p)) } };
  const m = path.match(/^\/pricing\/([^/]+)(\/changes|\/preview)?$/);
  if (!m) return null;
  const brand = decodeURIComponent(m[1] ?? "");
  const rest = m[2] ?? "";
  const p = brands.get(brand);
  if (!p) return { status: 404, body: { error: "not found" } };
  if (rest === "" && method === "GET") return { status: 200, body: item(brand, p) };
  if (rest === "/changes" && method === "GET") return { status: 200, body: { changes: journal.get(brand) ?? [] } };
  if (rest === "/preview" && method === "POST") {
    const problem = firstProblem(body.model, []);
    if (problem) return { status: 422, body: problem };
    const { model } = checkPricingModel(body.model);
    if (!model) throw new Error("checked above");
    const inputs = isObj(body.inputs) ? Object.fromEntries(Object.entries(body.inputs).map(([k, v]) => [k, String(v)])) : {};
    return { status: 200, body: { cents: priceOf(model, String(body.need), inputs)?.cents ?? null } };
  }
  if (rest !== "" || (method !== "PUT" && method !== "DELETE")) return { status: 405, body: { error: "method not allowed" } };
  if (role !== "admin") return { status: 403, body: { error: "your role may not do this" } };
  if (ALWAYS_CONFLICT || (body.expected_updated_at ?? null) !== p.updated_at) return { status: 409, body: { error: "stale", current: item(brand, p) } };
  if (method === "DELETE") {
    write(brand, p, me, null);
    return { status: 200, body: item(brand, p), changed: brand };
  }
  const problem = firstProblem(body.model, p.locales);
  if (problem) return { status: 422, body: problem };
  write(brand, p, me, body.model as Json);
  return { status: 200, body: item(brand, p), changed: brand };
}
