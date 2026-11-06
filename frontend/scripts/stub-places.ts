/**
 * Place settings for the dev stub: the live site data a place's page merges
 * over its baked config (SA-PANEL-PLACE-SETTINGS-SPEC.md, "Session API
 * contract"). In memory, validated by the contract's `checkPlaceLive`, with a
 * journal for the history and the reverts.
 *
 *   STUB_PLACES_CONFLICT=1   every save and revert answers 409, as if someone saved first
 */
import { randomUUID } from "node:crypto";

import { checkPlaceLive } from "@evinvest/sites-contract";

type Json = Record<string, unknown>;

export interface StubReply {
  status: number;
  body?: unknown;
}

interface StubPlace {
  settings: Json;
  updated_at: string | null;
  updated_by: string | null;
  withdrawn: boolean;
}

interface Change {
  id: string;
  at: string;
  by: string;
  before: Json;
  after: Json;
}

const ALWAYS_CONFLICT = process.env.STUB_PLACES_CONFLICT === "1";

const iso = () => new Date().toISOString();

/** Places with data, keyed `brand/slug`. Lyon 3 has every v1 field and one the panel does not edit. */
const places = new Map<string, StubPlace>([
  [
    "aquafix/lyon-3",
    {
      settings: {
        phone: "+33478000003",
        whatsapp: "+33600000003",
        telegram: "aquafix_devis_bot",
        hours: [
          { days: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"], opens: "08:00", closes: "19:00" },
          { days: ["Saturday"], opens: "09:00", closes: "12:00" },
        ],
        serviceArea: ["Lyon 3e", "Villeurbanne", "Bron"],
        booking: { default: "google_calendar", providers: { google_calendar: { url: "https://calendar.app.google/StubLyon3" }, link: { url: "https://book.example.fr/lyon-3" } } },
        address: { street: "12 rue Paul Bert", postalCode: "69003", locality: "Lyon" },
      },
      updated_at: new Date(Date.now() - 3 * 86_400_000).toISOString(),
      updated_by: "admin@example.test (stub)",
      withdrawn: false,
    },
  ],
]);

/** Added by hand: known to the panel with no lead yet. */
const added: { brand: string; location: string }[] = [{ brand: "vifnet", location: "paris-15" }];

const journal = new Map<string, Change[]>([
  [
    "aquafix/lyon-3",
    [
      {
        id: randomUUID(),
        at: new Date(Date.now() - 3 * 86_400_000).toISOString(),
        by: "admin@example.test (stub)",
        before: { phone: "+33478000000", address: { street: "12 rue Paul Bert", postalCode: "69003", locality: "Lyon" } },
        after: places.get("aquafix/lyon-3")?.settings ?? {},
      },
    ],
  ],
]);

const blank = (): StubPlace => ({ settings: {}, updated_at: null, updated_by: null, withdrawn: false });

function view(brand: string, slug: string, role: string): Json {
  const p = places.get(`${brand}/${slug}`) ?? blank();
  return { brand, slug, withdrawn: p.withdrawn, settings: p.settings, updated_at: p.updated_at, updated_by: p.updated_by, can_edit: role === "admin" };
}

/** The backend refuses rather than drops, so the editor sees why: one reason per path. */
function invalid(s: Json): Record<string, string> {
  return Object.fromEntries(checkPlaceLive(s).problems.map((p) => [p.path, `${p.code} (stub)`]));
}

function write(key: string, by: string, change: (p: StubPlace) => void): void {
  const p = places.get(key) ?? blank();
  const before = p.settings;
  change(p);
  p.updated_at = iso();
  p.updated_by = by;
  places.set(key, p);
  journal.set(key, [{ id: randomUUID(), at: p.updated_at, by, before, after: p.settings }, ...(journal.get(key) ?? [])]);
}

/** `/places` gains these per place. */
export function placeFlags(brand: string, location: string): { has_settings: boolean; withdrawn: boolean } {
  const p = places.get(`${brand}/${location}`);
  return { has_settings: !!p && Object.keys(p.settings).length > 0, withdrawn: p?.withdrawn ?? false };
}

/** Someone else saves the place (the live stub's doing): its phone flips between two numbers. */
export function touchPlace(brand: string, slug: string, by: string): void {
  write(`${brand}/${slug}`, by, (p) => {
    p.settings = { ...p.settings, phone: p.settings.phone === "+33478000003" ? "+33478000033" : "+33478000003" };
  });
}

export function addedPlaces(): { brand: string; location: string }[] {
  return added;
}

/**
 * The settings routes, or null when `path` (after `/api/v1`) is not one of them.
 * The caller has passed the gate and the CSRF check already.
 */
export function placeSettingsRoute(method: string, path: string, body: Json, role: string, me: string): StubReply | null {
  if (path === "/places" && method === "POST") {
    if (role !== "admin") return { status: 403, body: { error: "your role may not do this" } };
    const brand = String(body.brand ?? "");
    const location = String(body.slug ?? "");
    if (added.some((p) => p.brand === brand && p.location === location) || places.has(`${brand}/${location}`)) return { status: 409, body: { error: "exists" } };
    added.push({ brand, location });
    return { status: 201, body: view(brand, location, role) };
  }
  const m = path.match(/^\/places\/([^/]+)\/([^/]+)\/(settings(?:\/history|\/revert\/[^/]+)?|withdraw|restore)$/);
  if (!m) return null;
  const [brand, slug, rest] = [decodeURIComponent(m[1] ?? ""), decodeURIComponent(m[2] ?? ""), m[3] ?? ""];
  const key = `${brand}/${slug}`;
  const writes = method !== "GET";
  if (writes && role !== "admin") return { status: 403, body: { error: "your role may not do this" } };

  if (rest === "settings" && method === "GET") return { status: 200, body: view(brand, slug, role) };
  const stale = () => ALWAYS_CONFLICT || (body.expected_updated_at ?? null) !== (places.get(key)?.updated_at ?? null);
  if (rest === "settings" && method === "PUT") {
    if (stale()) return { status: 409, body: { error: "conflict" } };
    const settings = typeof body.settings === "object" && body.settings !== null ? (body.settings as Json) : {};
    const fields = invalid(settings);
    if (Object.keys(fields).length > 0) return { status: 422, body: { error: "invalid", fields } };
    write(key, me, (p) => (p.settings = settings));
    return { status: 200, body: view(brand, slug, role) };
  }
  if (rest === "settings/history" && method === "GET") return { status: 200, body: { changes: journal.get(key) ?? [] } };
  if (rest.startsWith("settings/revert/") && method === "POST") {
    const change = journal.get(key)?.find((c) => c.id === decodeURIComponent(rest.slice("settings/revert/".length)));
    if (!change) return { status: 404, body: { error: "not_found" } };
    if (stale()) return { status: 409, body: { error: "conflict" } };
    write(key, me, (p) => (p.settings = change.before));
    return { status: 200, body: view(brand, slug, role) };
  }
  if ((rest === "withdraw" || rest === "restore") && method === "POST") {
    const p = places.get(key) ?? blank();
    p.withdrawn = rest === "withdraw";
    places.set(key, p);
    return { status: 200, body: view(brand, slug, role) };
  }
  return { status: 405, body: { error: "method not allowed" } };
}
