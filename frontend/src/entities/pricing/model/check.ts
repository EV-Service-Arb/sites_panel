import {
  EFFECT_FIELD,
  EFFECT_MAX,
  INPUT_KINDS,
  PRICING_CURRENCY,
  PRICING_FORMAT,
  PRICING_LIMITS as LIMIT,
  type NeedPricing,
  type PricingInput,
  type PricingInputKind,
  type PricingLabels,
  type PricingModel,
} from "./model";

/**
 * The editor's copy of kitstart's validator (`core/pricing/validate.ts` at
 * 71d6451), held to the same fixtures (`tests/fixtures/pricing/`). It answers
 * codes rather than English, so the editor can say each in the reader's
 * language beside the field the path names.
 *
 * Paths are kitstart's without the leading `model.`: `inputs[0].options[2].labels.en`,
 * `needs.standard.inputs[1]`.
 */
export type ProblemCode =
  | "object"
  | "missing"
  | "unknownField"
  | "format"
  | "currency"
  | "date"
  | "int"
  | "slug"
  | "labels"
  | "locale"
  | "label"
  | "list"
  | "duplicate"
  | "kind"
  | "needKind"
  | "noInput"
  | "dearest"
  | "labelMissing";

export interface PricingProblem {
  path: string;
  code: ProblemCode;
  vars: Readonly<Record<string, string | number>>;
}

type Json = Record<string, unknown>;
const isObject = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);
const at = (path: string, key: string) => (path === "" ? key : `${path}.${key}`);

class Check {
  readonly problems: PricingProblem[] = [];
  fail(path: string, code: ProblemCode, vars: Record<string, string | number> = {}): void {
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
    if (typeof v !== "string" || !LIMIT.slug.test(v)) {
      this.fail(path, "slug");
      return "";
    }
    return v;
  }
  labels(path: string, v: unknown): PricingLabels {
    if (!isObject(v) || Object.keys(v).length === 0) {
      this.fail(path, "labels");
      return {};
    }
    const out: Record<string, string> = {};
    for (const [locale, text] of Object.entries(v)) {
      if (!LIMIT.locale.test(locale)) this.fail(at(path, locale), "locale");
      else if (typeof text !== "string" || text.trim() === "" || text.length > LIMIT.maxLabel) this.fail(at(path, locale), "label", { max: LIMIT.maxLabel });
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

/** `YYYY-MM-DD`, and a day the calendar has. */
export function isDay(v: unknown): v is string {
  if (typeof v !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(v)) return false;
  const day = new Date(`${v}T00:00:00Z`);
  return !Number.isNaN(day.getTime()) && day.toISOString().slice(0, 10) === v;
}

const isKind = (v: unknown): v is PricingInputKind => typeof v === "string" && (INPUT_KINDS as readonly string[]).includes(v);

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
    const there = `${path}.options[${i}]`;
    if (!isObject(o)) {
      c.fail(there, "object");
      return { id: "", labels: {}, value: 0 };
    }
    c.keys(there, o, ["id", "labels", effect]);
    return { id: c.slug(at(there, "id"), o.id), labels: c.labels(at(there, "labels"), o.labels), value: c.int(at(there, effect), o[effect], 0, EFFECT_MAX[kind]) };
  });
  c.unique(at(path, "options"), options.map((o) => o.id));
  switch (kind) {
    case "add":
      return { id, kind, labels, options: options.map((o) => ({ id: o.id, labels: o.labels, addCents: o.value })) };
    case "multiply":
      return { id, kind, labels, options: options.map((o) => ({ id: o.id, labels: o.labels, multiplyBp: o.value })) };
    case "discount":
      return { id, kind, labels, options: options.map((o) => ({ id: o.id, labels: o.labels, discountBp: o.value })) };
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
  const ids = c.list(at(path, "inputs"), v.inputs, 0, LIMIT.maxNeedInputs).map((id, i) => c.slug(`${path}.inputs[${i}]`, id));
  c.unique(at(path, "inputs"), ids);
  for (const [i, id] of ids.entries()) if (id !== "" && !inputs.some((x) => x.id === id)) c.fail(`${path}.inputs[${i}]`, "noInput", { id });
  return { kind: "estimate", baseCents, inputs: ids };
}

/** `cents × bp / 10 000` rounded half up, on integers: what kitstart's walk of the dearest answer uses. */
function mulBp(cents: number, bp: number): number {
  const n = cents * bp + 5_000;
  return (n - (n % 10_000)) / 10_000;
}

/**
 * The cap on the dearest answer to every input, as kitstart checks it: each
 * effect only raises the price with its value, so if no step of the dearest
 * walk passes the cap, no combination does. Validation, not pricing — the
 * price itself only the server tells (`/preview`).
 */
function dearest(c: Check, path: string, pricing: Extract<NeedPricing, { kind: "estimate" }>, inputs: readonly PricingInput[]): void {
  const asked = pricing.inputs.map((id) => inputs.find((i) => i.id === id)).filter((i): i is PricingInput => i !== undefined);
  const max = (i: PricingInput) => Math.max(...i.options.map((o) => ("addCents" in o ? o.addCents : "multiplyBp" in o ? o.multiplyBp : 0)));
  let total = pricing.baseCents;
  const steps = [total];
  for (const i of asked) if (i.kind === "add") steps.push((total += max(i)));
  for (const i of asked) if (i.kind === "multiply") steps.push((total = mulBp(total, max(i))));
  if (steps.some((s) => s > LIMIT.maxPriceCents)) c.fail(path, "dearest", { max: LIMIT.maxPriceCents });
}

/** Every reason `value` is not a pricing model, and the model rebuilt from what was checked when there is none. */
export function checkPricingModel(value: unknown): { problems: PricingProblem[]; model: PricingModel | null } {
  const c = new Check();
  if (!isObject(value)) return { problems: [{ path: "", code: "object", vars: {} }], model: null };
  c.keys("", value, ["format", "currency", "validFrom", "roundToCents", "minimumCents", "inputs", "needs"]);
  if (value.format !== PRICING_FORMAT) c.fail("format", "format", { format: PRICING_FORMAT });
  if (value.currency !== PRICING_CURRENCY) c.fail("currency", "currency", { currency: PRICING_CURRENCY });
  if (!isDay(value.validFrom)) c.fail("validFrom", "date");
  const roundToCents = c.int("roundToCents", value.roundToCents, 1, LIMIT.maxPriceCents);
  const minimumCents = c.int("minimumCents", value.minimumCents, 0, LIMIT.maxPriceCents);
  const inputs = c
    .list("inputs", value.inputs, 0, LIMIT.maxInputs)
    .map((v, i) => input(c, `inputs[${i}]`, v))
    .filter((i): i is PricingInput => i !== null);
  c.unique("inputs", inputs.map((i) => i.id));
  const needs: Record<string, NeedPricing> = {};
  if (!isObject(value.needs)) c.fail("needs", "object");
  else {
    const entries = Object.entries(value.needs);
    if (entries.length > LIMIT.maxNeeds) c.fail("needs", "list", { min: 0, max: LIMIT.maxNeeds });
    for (const [slug, v] of entries) {
      const path = `needs.${slug}`;
      if (!LIMIT.slug.test(slug)) c.fail(path, "slug");
      const pricing = need(c, path, v, inputs);
      if (pricing) needs[slug] = pricing;
    }
  }
  if (c.problems.length > 0) return { problems: c.problems, model: null };
  for (const [slug, pricing] of Object.entries(needs)) if (pricing.kind === "estimate") dearest(c, `needs.${slug}`, pricing, inputs);
  if (c.problems.length > 0) return { problems: c.problems, model: null };
  return { problems: [], model: { format: PRICING_FORMAT, currency: PRICING_CURRENCY, validFrom: String(value.validFrom), roundToCents, minimumCents, inputs, needs } };
}

/**
 * A label missing in one of the brand's locales: the server takes such a
 * model, but a site would show an option with no words, so the editor
 * refuses to save it.
 */
export function missingLabels(model: PricingModel, locales: readonly string[]): PricingProblem[] {
  const out: PricingProblem[] = [];
  const labelled = (path: string, labels: PricingLabels) => {
    for (const locale of locales) if (!Object.hasOwn(labels, locale)) out.push({ path: `${path}.labels.${locale}`, code: "labelMissing", vars: { locale } });
  };
  for (const [i, inp] of model.inputs.entries()) {
    labelled(`inputs[${i}]`, inp.labels);
    for (const [j, o] of inp.options.entries()) labelled(`inputs[${i}].options[${j}]`, o.labels);
  }
  return out;
}
