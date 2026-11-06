// `contracts/vectors`, read in place: what `panel_core` is held to as well.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  LEAD_LIMITS,
  bookingRequested,
  checkBookingConfig,
  checkBookingUrl,
  checkPlaceLive,
  checkPricingModel,
  declarationProblem,
  ingestBody,
  isLeadRef,
  isOpaqueId,
  messageRefOf,
  normalizePhone,
  preferredDateInWindow,
  priceOf,
  siteEvent,
  validHoldout,
  validWeights,
  type ExperimentDeclaration,
  type PageProvider,
  type PricingInputs,
  type SiteEventKey,
  type SiteProperties,
  type Subject,
} from "../src/index.js";

const DIR = join(import.meta.dirname, "../../../contracts/vectors");
const read = (path: string): unknown => JSON.parse(readFileSync(join(DIR, path), "utf8"));
const each = (dir: string): [string, any][] => {
  const files = readdirSync(join(DIR, dir)).filter(f => f.endsWith(".json")).sort();
  expect(files.length).toBeGreaterThan(0);
  return files.map(f => [f, read(join(dir, f))]);
};
const list = (file: string) => read(file) as any[];
type Problem = { path: string; code: string };
const set = (problems: readonly Problem[]) => problems.map(p => `${p.path} ${p.code}`).sort();

describe("pricing", () => {
  it.each(each("pricing/valid"))("%s is accepted and written back unchanged", (_, model) => {
    const checked = checkPricingModel(model);
    expect(checked.problems).toEqual([]);
    expect(checked.model).toEqual(model);
  });

  it.each(each("pricing/invalid"))("%s is refused for what it says", (_, v) => {
    expect(set(checkPricingModel(v.model).problems)).toEqual(set(v.problems));
  });

  it.each(list("pricing/locales.json").map(c => [c.name, c]))("%s", (_, c) => {
    expect(set(checkPricingModel(c.model, c.locales).problems)).toEqual(set(c.problems));
  });

  it.each(list("pricing/cases.json").map(c => [c.name, c]))("%s is priced to the cent", (_, c) => {
    const { model } = checkPricingModel(c.model);
    const price = priceOf(model!, c.need, c.inputs as PricingInputs);
    expect(price?.cents ?? null).toBe(c.cents);
    if (price) expect(price.breakdown.reduce((sum, line) => sum + line.cents, 0)).toBe(price.cents);
  });
});

describe("booking", () => {
  it.each(each("booking/valid"))("%s is kept as it came", (_, config) => {
    expect(checkBookingConfig(config)).toEqual({ problems: [], config });
  });

  it.each(each("booking/invalid"))("%s is refused for what it says", (_, v) => {
    const checked = checkBookingConfig(v.config);
    expect(checked.config).toBeNull();
    expect(set(checked.problems)).toEqual(set(v.problems));
  });

  it.each(list("booking/urls.json").map(u => [`${u.provider} ${u.url}`, u]))("%s", (_, u) => {
    expect(checkBookingUrl(u.provider as PageProvider, u.url)).toBe(u.problem);
  });

  it.each(each("booking/requested/valid"))("requested %s is taken", (_, props) => {
    expect(bookingRequested(props)).toEqual(props);
  });

  it.each(each("booking/requested/invalid"))("requested %s is refused", (_, props) => {
    expect(bookingRequested(props)).toBeNull();
  });
});

describe("phone", () => {
  it.each(list("phone.json").map(p => [p.raw, p.e164]))("%j reads as %s", (raw, e164) => {
    expect(normalizePhone(raw)).toBe(e164);
  });
});

describe("message ref", () => {
  it.each(list("message-ref.json").map(v => [v.typed, v.ref]))("%j reads as %s", (typed, ref) => {
    expect(messageRefOf(typed)).toBe(ref);
  });
});

describe("an override a landing takes or drops", () => {
  const o = read("overrides.json") as { weights: { weights: number[]; variants: number; ok: boolean }[]; holdout: { value: number; ok: boolean }[] };
  it.each(o.weights.map(v => [v.weights, v.variants, v.ok]))("weights %j for %i variants: %s", (weights, variants, ok) => {
    expect(validWeights(weights, variants) !== undefined).toBe(ok);
  });
  it.each(o.holdout.map(v => [v.value, v.ok]))("holdout %d: %s", (value, ok) => {
    expect(validHoldout(value) !== undefined).toBe(ok);
  });
});

describe("place", () => {
  it.each(list("place.json").map(c => [c.name, c]))("%s", (_, c) => {
    const { live, problems } = checkPlaceLive(c.settings);
    expect(live).toEqual(c.live);
    expect(set(problems)).toEqual(set(c.problems));
  });
});

describe("ingest", () => {
  it.each(each("ingest").filter(([f]) => f !== "declarations.json"))("%s is written byte for byte", (_, v) => {
    const head = { id: v.head.id as string, occurredAt: new Date(v.head.occurredAt), sourceId: v.head.sourceId as string, subject: v.head.subject as Subject };
    const key = v.key as SiteEventKey;
    const body = ingestBody(siteEvent(key, head, v.properties as SiteProperties[typeof key], v.pii));
    expect(JSON.stringify(body)).toBe(JSON.stringify(v.body));
  });

  it.each(list("ingest/declarations.json").map(c => [c.name, c]))("declaring %s", (_, c) => {
    expect(declarationProblem(c.declaration as ExperimentDeclaration) === null).toBe(c.ok);
  });

  const ids = read("ids.json") as Record<string, { value: string; ok: boolean }[]>;
  const patterns: Record<string, (s: string) => boolean> = {
    opaque: isOpaqueId,
    analyticsId: s => new RegExp(LEAD_LIMITS.analyticsId).test(s),
    messageRef: s => new RegExp(LEAD_LIMITS.messageRef).test(s),
    leadRef: isLeadRef,
  };
  it.each(Object.entries(ids).flatMap(([kind, vs]) => vs.map(v => [kind, v.value, v.ok] as const)))("%s %j: %s", (kind, value, ok) => {
    expect(patterns[kind]!(value)).toBe(ok);
  });
});

describe("a preferred day", () => {
  it.each(list("booking/window.json").map(c => [c.date, c.today, c.ok]))("%s asked on %s: %s", (date, today, ok) => {
    expect(preferredDateInWindow(date, today)).toBe(ok);
  });
});
