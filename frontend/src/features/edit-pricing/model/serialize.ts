import { PRICING_CURRENCY, PRICING_FORMAT, type PricingCode, type PricingModel, checkPricingModel } from "@evinvest/sites-contract";

import { parseHundredths } from "@/entities/pricing";

import type { PricingDraft } from "./draft";
import { FIELD, within } from "./fields";
import { EFFECT_FIELD, fieldOfPath } from "./paths";

export type DraftCode = PricingCode | "required" | "money" | "percent";

/** One thing wrong with the draft, filed under its field (null: the model as a whole). */
export interface DraftProblem {
  field: string | null;
  code: DraftCode;
  vars: Readonly<Record<string, string | number>>;
}

export interface Serialized {
  /**
   * The model the draft says, when it says one the site could price: every
   * amount typed as a number and the shape valid. A label still missing does
   * not stop a preview, only a save.
   */
  model: PricingModel | null;
  /** Empty when the draft may be saved as it is. */
  problems: DraftProblem[];
}

const problem = (field: string | null, code: DraftCode, vars: Record<string, string | number> = {}): DraftProblem => ({ field, code, vars });

/** Labels with words in them, trimmed; a blank one is no label (the contract refuses blanks). */
function labelsOf(labels: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(labels).flatMap(([locale, text]) => (text.trim() === "" ? [] : [[locale, text.trim()]])));
}

/** Text typed as hundredths (euros, percents): its value, or the reason it has none. */
function amount(out: DraftProblem[], field: string, raw: string, unit: "money" | "percent"): number | null {
  if (raw.trim() === "") {
    out.push(problem(field, "required"));
    return null;
  }
  const n = parseHundredths(raw);
  if (n === null) out.push(problem(field, unit));
  return n;
}

function labelled(out: DraftProblem[], labels: Record<string, string>, locales: readonly string[], field: (locale: string) => string): void {
  for (const locale of locales) if ((labels[locale] ?? "").trim() === "") out.push(problem(field(locale), "labelMissing", { locale }));
}

function slugged(out: DraftProblem[], id: string, field: string): void {
  if (id.trim() === "") out.push(problem(field, "required"));
}

/**
 * The draft as the model a site reads — exactly its fields, integer cents and
 * basis points — with every problem the editor can name before the server
 * does: what the draft cannot even say (an empty slug, "12,345 €", a label
 * missing in one of the brand's `locales`) and then the contract's own rules on
 * what it says.
 */
export function modelOf(draft: PricingDraft, locales: readonly string[]): Serialized {
  const local: DraftProblem[] = [];
  if (draft.validFrom.trim() === "") local.push(problem(FIELD.validFrom, "required"));
  const roundToCents = amount(local, FIELD.roundTo, draft.roundTo, "money");
  const minimumCents = amount(local, FIELD.minimum, draft.minimum, "money");

  const inputs = draft.inputs.map((input) => {
    slugged(local, input.id, FIELD.inputId(input.key));
    labelled(local, input.labels, locales, (l) => FIELD.inputLabel(input.key, l));
    const effect = EFFECT_FIELD[input.kind];
    const options = input.options.map((o) => {
      slugged(local, o.id, FIELD.optionId(o.key));
      labelled(local, o.labels, locales, (l) => FIELD.optionLabel(o.key, l));
      const value = amount(local, FIELD.optionValue(o.key), o.value, input.kind === "add" ? "money" : "percent");
      return { id: o.id.trim(), labels: labelsOf(o.labels), [effect]: value ?? 0 };
    });
    return { id: input.id.trim(), kind: input.kind, labels: labelsOf(input.labels), options };
  });

  const idOf = (key: string) => draft.inputs.find((i) => i.key === key)?.id.trim() ?? "";
  const needs: Record<string, unknown> = {};
  for (const need of draft.needs) {
    const id = need.id.trim();
    const cents = amount(local, FIELD.needAmount(need.key), need.amount, "money");
    if (id === "") local.push(problem(FIELD.needId(need.key), "required"));
    else if (Object.hasOwn(needs, id)) local.push(problem(FIELD.needId(need.key), "duplicate", { id }));
    else
      needs[id] =
        need.kind === "fixed"
          ? { kind: "fixed", cents: cents ?? 0 }
          : { kind: "estimate", baseCents: cents ?? 0, inputs: need.inputs.map(idOf).filter((x) => x !== "") };
  }

  const candidate = { format: PRICING_FORMAT, currency: PRICING_CURRENCY, validFrom: draft.validFrom, roundToCents: roundToCents ?? 1, minimumCents: minimumCents ?? 0, inputs, needs };
  const checked = checkPricingModel(candidate);
  // A field the draft already faults (or one inside it) needs no second, vaguer reason.
  const rules = checked.problems
    .map((p) => problem(fieldOfPath(p.path, draft), p.code, p.vars))
    .filter((p) => p.field === null || !local.some((l) => l.field !== null && within(l.field, p.field ?? "")));
  const model = checked.model && local.every((p) => p.code === "labelMissing") ? checked.model : null;
  return { model, problems: [...local, ...rules] };
}
