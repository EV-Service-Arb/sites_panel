import type { PricingInput, PricingInputKind } from "@evinvest/sites-contract";

import type { InputDraft, PricingDraft } from "./draft";
import { FIELD } from "./fields";

type Segment = string | number;

/** `inputs[2].options.studio.labels.en` → `["inputs", 2, "options", "studio", "labels", "en"]`. */
export function segmentsOf(path: string): Segment[] {
  const out: Segment[] = [];
  for (const part of path.split(".")) {
    const m = part.match(/^([^[\]]*)((?:\[\d+\])*)$/);
    if (!m) return out;
    if (m[1]) out.push(m[1]);
    for (const index of (m[2] ?? "").matchAll(/\[(\d+)\]/g)) out.push(Number(index[1]));
  }
  return out;
}

/** A row named by its index (`inputs[2]`) or by its slug (`inputs.bedrooms`). */
const pick = <T extends { id: string }>(rows: readonly T[], at: Segment | undefined): T | undefined =>
  typeof at === "number" ? rows[at] : at === undefined ? undefined : rows.find((r) => r.id === at);

/** The field an option of each kind carries its effect in. */
export const EFFECT_FIELD = { add: "addCents", multiply: "multiplyBp", discount: "discountBp" } as const satisfies {
  [K in PricingInputKind]: Exclude<keyof Extract<PricingInput, { kind: K }>["options"][number], "id" | "labels">;
};

const EFFECTS = new Set<string>(Object.values(EFFECT_FIELD));

function inInput(input: InputDraft, rest: Segment[]): string {
  const [head, sel, ...tail] = rest;
  if (head === "id") return FIELD.inputId(input.key);
  if (head === "kind") return FIELD.inputKind(input.key);
  if (head === "labels") return typeof sel === "string" ? FIELD.inputLabel(input.key, sel) : FIELD.inputLabels(input.key);
  if (head !== "options") return FIELD.input(input.key);
  const option = pick(input.options, sel);
  if (!option) return FIELD.inputOptions(input.key);
  const [field, locale] = tail;
  if (field === "id") return FIELD.optionId(option.key);
  if (field === "labels") return typeof locale === "string" ? FIELD.optionLabel(option.key, locale) : FIELD.optionLabels(option.key);
  if (typeof field === "string" && EFFECTS.has(field)) return FIELD.optionValue(option.key);
  return FIELD.option(option.key);
}

const GENERAL: Record<string, string> = { validFrom: FIELD.validFrom, roundToCents: FIELD.roundTo, minimumCents: FIELD.minimum };

/**
 * The editor field a model path names — the contract's and the server's paths
 * alike, by index or by slug — or the nearest row or list that holds it.
 * Null for what the editor has no field for (`format`, `currency`, the model
 * as a whole): those are told above the form.
 */
export function fieldOfPath(path: string, draft: PricingDraft): string | null {
  const [head, sel, ...rest] = segmentsOf(path);
  if (typeof head !== "string") return null;
  if (head in GENERAL) return GENERAL[head] ?? null;
  if (head === "inputs") {
    const input = pick(draft.inputs, sel);
    return input ? inInput(input, rest) : FIELD.inputs;
  }
  if (head === "needs") {
    const need = typeof sel === "string" ? draft.needs.find((n) => n.id === sel) : undefined;
    if (!need) return FIELD.needs;
    const [field] = rest;
    if (field === "kind") return FIELD.needKind(need.key);
    if (field === "baseCents" || field === "cents") return FIELD.needAmount(need.key);
    if (field === "inputs") return FIELD.needInputs(need.key);
    return FIELD.need(need.key);
  }
  return null;
}
