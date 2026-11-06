import type { PricingCode } from "./generated/codes.js";
import { INPUT_KINDS, PRICING_CURRENCY, PRICING_FORMAT, PRICING_LIMITS as LIMIT, type PricingInputKind } from "./generated/pricing.js";
import type { NeedPricing, PricingInput, PricingModel } from "./generated/types.js";
import { effects, mulBp, type PricingLabels } from "./price.js";
import { at, isDate, isObject, trim, type Json } from "./text.js";

/**
 * One thing wrong with a model: its path from the model's root (`inputs[0].options[2].addCents`,
 * `""` for the model itself), the code the panel names it by too, and what the code's words
 * need (`{ min, max }`, `{ id }`, `{ locale }`).
 */
export interface PricingProblem {
  path: string;
  code: PricingCode;
  vars: Readonly<Record<string, string | number>>;
}

const SLUG = /* @__PURE__ */ new RegExp(LIMIT.slug);
const LOCALE = /* @__PURE__ */ new RegExp(LIMIT.locale);

const EFFECT_FIELD = { add: "addCents", multiply: "multiplyBp", discount: "discountBp" } as const satisfies Record<PricingInputKind, string>;
const EFFECT_MAX = { add: LIMIT.maxPriceCents, multiply: LIMIT.maxMultiplyBp, discount: LIMIT.maxDiscountBp } as const satisfies Record<PricingInputKind, number>;

class Check {
  readonly problems: PricingProblem[] = [];
  fail(path: string, code: PricingCode, vars: Record<string, string | number> = {}): void {
    this.problems.push({ path, code, vars });
  }
  keys(path: string, v: Json, required: readonly string[]): void {
    for (const key of required) if (!Object.hasOwn(v, key)) this.fail(at(path, key), "missing");
    for (const key of Object.keys(v)) if (!required.includes(key)) this.fail(at(path, key), "unknownField");
  }
  int(path: string, v: unknown, min: number, max: number): number {
    if (typeof v !== "number" || !Number.isSafeInteger(v) || v < min || v > max) {
      this.fail(path, "int", { min, max });
      return min;
    }
    return v;
  }
  slug(path: string, v: unknown): string {
    if (typeof v !== "string" || !SLUG.test(v)) {
      this.fail(path, "slug");
      return "";
    }
    return v;
  }
  labels(path: string, v: unknown): Record<string, string> {
    if (!isObject(v) || Object.keys(v).length === 0) {
      this.fail(path, "labels");
      return {};
    }
    const out: Record<string, string> = {};
    for (const [locale, text] of Object.entries(v)) {
      if (!LOCALE.test(locale)) this.fail(at(path, locale), "locale");
      else if (typeof text !== "string" || trim(text) === "" || text.length > LIMIT.maxLabel) this.fail(at(path, locale), "label", { max: LIMIT.maxLabel });
      else out[locale] = text;
    }
    return out;
  }
  list(path: string, v: unknown, min: number, max: number): unknown[] {
    if (!Array.isArray(v) || v.length < min || v.length > max) {
      this.fail(path, "list", { min, max });
      return [];
    }
    return v;
  }
  unique(path: string, ids: readonly string[]): void {
    const dup = ids.find((id, i) => id !== "" && ids.indexOf(id) !== i);
    if (dup !== undefined) this.fail(path, "duplicate", { id: dup });
  }
}

const isKind = (v: unknown): v is PricingInputKind => INPUT_KINDS.some(k => k === v);

function input(c: Check, path: string, v: unknown): PricingInput | null {
  if (!isObject(v)) {
    c.fail(path, "object");
    return null;
  }
  c.keys(path, v, ["id", "kind", "labels", "options"]);
  const id = c.slug(at(path, "id"), v.id);
  const labels = c.labels(at(path, "labels"), v.labels);
  const kind = v.kind;
  if (!isKind(kind)) {
    c.fail(at(path, "kind"), "kind");
    return null;
  }
  const effect = EFFECT_FIELD[kind];
  const options = c.list(at(path, "options"), v.options, 1, LIMIT.maxOptions).map((o, i) => {
    const there = at(at(path, "options"), i);
    if (!isObject(o)) {
      c.fail(there, "object");
      return { id: "", labels: {}, value: 0 };
    }
    c.keys(there, o, ["id", "labels", effect]);
    return { id: c.slug(at(there, "id"), o.id), labels: c.labels(at(there, "labels"), o.labels), value: c.int(at(there, effect), o[effect], 0, EFFECT_MAX[kind]) };
  });
  c.unique(at(path, "options"), options.map(o => o.id));
  switch (kind) {
    case "add":
      return { kind, id, labels, options: options.map(o => ({ id: o.id, labels: o.labels, addCents: o.value })) };
    case "multiply":
      return { kind, id, labels, options: options.map(o => ({ id: o.id, labels: o.labels, multiplyBp: o.value })) };
    case "discount":
      return { kind, id, labels, options: options.map(o => ({ id: o.id, labels: o.labels, discountBp: o.value })) };
  }
}

function need(c: Check, path: string, v: unknown, inputs: readonly PricingInput[]): NeedPricing | null {
  if (!isObject(v)) {
    c.fail(path, "object");
    return null;
  }
  if (v.kind === "fixed") {
    c.keys(path, v, ["kind", "cents"]);
    return { kind: "fixed", cents: c.int(at(path, "cents"), v.cents, 0, LIMIT.maxPriceCents) };
  }
  if (v.kind !== "estimate") {
    c.fail(at(path, "kind"), "needKind");
    return null;
  }
  c.keys(path, v, ["kind", "baseCents", "inputs"]);
  const baseCents = c.int(at(path, "baseCents"), v.baseCents, 0, LIMIT.maxPriceCents);
  const ids = c.list(at(path, "inputs"), v.inputs, 0, LIMIT.maxNeedInputs).map((id, i) => c.slug(at(at(path, "inputs"), i), id));
  c.unique(at(path, "inputs"), ids);
  for (const [i, id] of ids.entries()) if (id !== "" && !inputs.some(x => x.id === id)) c.fail(at(at(path, "inputs"), i), "noInput", { id });
  return { kind: "estimate", baseCents, inputs: ids };
}

/**
 * The dearest answer to every input — each effect only raises the price with its value, and
 * rounding half up is monotonic — walked step by step: if no step of it passes the cap, no
 * combination does.
 */
function dearest(c: Check, path: string, pricing: Extract<NeedPricing, { kind: "estimate" }>, inputs: readonly PricingInput[]): void {
  const asked = pricing.inputs.map(id => inputs.find(i => i.id === id)).filter(i => i !== undefined);
  const max = (i: PricingInput) => (i.kind === "discount" ? 0 : Math.max(...effects(i).map(o => o.effect)));
  let total = pricing.baseCents;
  let over = total > LIMIT.maxPriceCents;
  for (const i of asked.filter(i => i.kind === "add")) over ||= (total += max(i)) > LIMIT.maxPriceCents;
  for (const i of asked.filter(i => i.kind === "multiply")) over ||= (total = mulBp(total, max(i))) > LIMIT.maxPriceCents;
  if (over) c.fail(path, "dearest", { max: LIMIT.maxPriceCents });
}

/**
 * Every reason `value` is not a pricing model, and the model rebuilt from what was checked when
 * there is none: taken whole or not at all, unknown fields refused rather than ignored. With
 * `locales`, a model a site can show: every input and option labelled in each of them.
 */
export function checkPricingModel(value: unknown, locales: readonly string[] = []): { problems: PricingProblem[]; model: PricingModel | null } {
  const c = new Check();
  if (!isObject(value)) return { problems: [{ path: "", code: "object", vars: {} }], model: null };
  c.keys("", value, ["format", "currency", "validFrom", "roundToCents", "minimumCents", "inputs", "needs"]);
  if (value.format !== PRICING_FORMAT) c.fail("format", "format", { format: PRICING_FORMAT });
  if (value.currency !== PRICING_CURRENCY) c.fail("currency", "currency", { currency: PRICING_CURRENCY });
  if (!isDate(value.validFrom)) c.fail("validFrom", "date");
  const roundToCents = c.int("roundToCents", value.roundToCents, 1, LIMIT.maxPriceCents);
  const minimumCents = c.int("minimumCents", value.minimumCents, 0, LIMIT.maxPriceCents);
  const inputs = c
    .list("inputs", value.inputs, 0, LIMIT.maxInputs)
    .map((v, i) => input(c, at("inputs", i), v))
    .filter(i => i !== null);
  c.unique("inputs", inputs.map(i => i.id));
  const needs: Record<string, NeedPricing> = {};
  if (!isObject(value.needs)) c.fail("needs", "object");
  else {
    const entries = Object.entries(value.needs);
    if (entries.length > LIMIT.maxNeeds) c.fail("needs", "list", { min: 0, max: LIMIT.maxNeeds });
    for (const [slug, v] of entries) {
      const path = at("needs", slug);
      if (!SLUG.test(slug)) c.fail(path, "slug");
      const pricing = need(c, path, v, inputs);
      if (pricing) needs[slug] = pricing;
    }
  }
  if (c.problems.length > 0) return { problems: c.problems, model: null };
  for (const [slug, pricing] of Object.entries(needs)) if (pricing.kind === "estimate") dearest(c, at("needs", slug), pricing, inputs);
  if (c.problems.length > 0) return { problems: c.problems, model: null };
  const labelled = (path: string, labels: PricingLabels) => {
    for (const locale of locales) if (!Object.hasOwn(labels, locale)) c.fail(at(at(path, "labels"), locale), "labelMissing", { locale });
  };
  for (const [i, inp] of inputs.entries()) {
    labelled(at("inputs", i), inp.labels);
    for (const [j, o] of inp.options.entries()) labelled(at(at(at("inputs", i), "options"), j), o.labels);
  }
  if (c.problems.length > 0) return { problems: c.problems, model: null };
  return { problems: [], model: { format: PRICING_FORMAT, currency: PRICING_CURRENCY, validFrom: String(value.validFrom), roundToCents, minimumCents, inputs, needs } };
}
