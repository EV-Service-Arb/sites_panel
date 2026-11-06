import type { PricingInputKind } from "@evinvest/sites-contract";

import { type InputDraft, type NeedDraft, type OptionDraft, type PricingDraft, emptyInput, emptyNeed, emptyOption } from "./draft";

/** Edits of a draft, each a new draft: the editor's rows call these and nothing else. */

const swap = <T,>(list: readonly T[], from: number, to: number): T[] => {
  const out = [...list];
  const [item] = out.splice(from, 1);
  if (item !== undefined) out.splice(to, 0, item);
  return out;
};

export function updateInput(d: PricingDraft, key: string, patch: Partial<Omit<InputDraft, "key" | "kind" | "options">>): PricingDraft {
  return { ...d, inputs: d.inputs.map((i) => (i.key === key ? { ...i, ...patch } : i)) };
}

/** A new kind means new units: the options start again from "no change" rather than reading euros as percents. */
export function setInputKind(d: PricingDraft, key: string, kind: PricingInputKind): PricingDraft {
  return {
    ...d,
    inputs: d.inputs.map((i) => (i.key !== key || i.kind === kind ? i : { ...i, kind, options: i.options.map((o) => ({ ...o, value: emptyOption(kind).value })) })),
  };
}

export const addInput = (d: PricingDraft): PricingDraft => ({ ...d, inputs: [...d.inputs, emptyInput()] });

/** A removed input is no longer asked by any need. */
export function removeInput(d: PricingDraft, key: string): PricingDraft {
  return { ...d, inputs: d.inputs.filter((i) => i.key !== key), needs: d.needs.map((n) => ({ ...n, inputs: n.inputs.filter((k) => k !== key) })) };
}

const withOptions = (d: PricingDraft, inputKey: string, change: (options: OptionDraft[], input: InputDraft) => OptionDraft[]): PricingDraft => ({
  ...d,
  inputs: d.inputs.map((i) => (i.key === inputKey ? { ...i, options: change(i.options, i) } : i)),
});

export const addOption = (d: PricingDraft, inputKey: string): PricingDraft => withOptions(d, inputKey, (os, i) => [...os, emptyOption(i.kind)]);

export const removeOption = (d: PricingDraft, inputKey: string, key: string): PricingDraft => withOptions(d, inputKey, (os) => os.filter((o) => o.key !== key));

export const updateOption = (d: PricingDraft, inputKey: string, key: string, patch: Partial<Omit<OptionDraft, "key">>): PricingDraft =>
  withOptions(d, inputKey, (os) => os.map((o) => (o.key === key ? { ...o, ...patch } : o)));

export const addNeed = (d: PricingDraft): PricingDraft => ({ ...d, needs: [...d.needs, emptyNeed()] });

export const removeNeed = (d: PricingDraft, key: string): PricingDraft => ({ ...d, needs: d.needs.filter((n) => n.key !== key) });

export const updateNeed = (d: PricingDraft, key: string, patch: Partial<Omit<NeedDraft, "key">>): PricingDraft => ({
  ...d,
  needs: d.needs.map((n) => (n.key === key ? { ...n, ...patch } : n)),
});

/** Asked or not: a newly asked input goes last, as the form asks them in order. */
export function toggleNeedInput(d: PricingDraft, needKey: string, inputKey: string): PricingDraft {
  return {
    ...d,
    needs: d.needs.map((n) => (n.key !== needKey ? n : { ...n, inputs: n.inputs.includes(inputKey) ? n.inputs.filter((k) => k !== inputKey) : [...n.inputs, inputKey] })),
  };
}

/** One place earlier (-1) or later (+1) in the order the form asks. */
export function moveNeedInput(d: PricingDraft, needKey: string, inputKey: string, by: -1 | 1): PricingDraft {
  return {
    ...d,
    needs: d.needs.map((n) => {
      if (n.key !== needKey) return n;
      const from = n.inputs.indexOf(inputKey);
      const to = from + by;
      return from < 0 || to < 0 || to >= n.inputs.length ? n : { ...n, inputs: swap(n.inputs, from, to) };
    }),
  };
}
