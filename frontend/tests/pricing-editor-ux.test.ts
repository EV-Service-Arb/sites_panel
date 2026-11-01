import { readFileSync } from "node:fs";

import { type ReactElement, createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { type PricingItem, type PricingModel, PricingStatus, changedParts, pricingStamp } from "@/entities/pricing";
import { type PricingEditor, PricingEditorForm } from "@/features/edit-pricing";
import { PASTE_FIELD_CLASS, PROBLEMS_CLASS } from "@/features/edit-pricing/config/paste";
import { draftOf } from "@/features/edit-pricing/model/draft";
import { NO_FIELD_ERRORS, type FieldErrors } from "@/features/edit-pricing/model/errors";
import { FIELD, describedByOf, hintIdOf, messageIdOf } from "@/features/edit-pricing/model/fields";
import { effectHint } from "@/features/edit-pricing/model/hints";
import { modelOf } from "@/features/edit-pricing/model/serialize";
import { ConflictAlert } from "@/features/edit-pricing/ui/conflict-alerts";
import { TextField } from "@/features/edit-pricing/ui/text-field";
import { LocaleProvider } from "@/shared/i18n";
import { formatMoment } from "@/shared/lib/format";

const render = (element: ReactElement) => renderToStaticMarkup(createElement(LocaleProvider, { value: "en" }, element));
const cleaning = JSON.parse(readFileSync(new URL("./fixtures/pricing/valid/cleaning.json", import.meta.url), "utf8")) as PricingModel;
const item = (model: PricingModel | null, updated_at: string | null = "2026-10-03T12:34:56Z"): PricingItem => ({
  brand_id: "vifnet",
  locales: ["fr", "en"],
  model,
  updated_at,
  updated_by: "admin@example.test",
});

/** An editor over `base` that does nothing when asked: enough to render the form. */
function editorOf(base: PricingItem, errors: FieldErrors = NO_FIELD_ERRORS): PricingEditor {
  const draft = draftOf(base.model, "2026-10-03");
  const noop = () => {};
  return {
    base,
    draft,
    update: noop,
    serialized: modelOf(draft, base.locales),
    errors,
    changed: false,
    state: { kind: "idle" },
    save: noop,
    overwrite: noop,
    replace: noop,
    reset: noop,
    showPath: noop,
    showFirst: noop,
  };
}

const attr = (tag: string, name: string): string | null => new RegExp(`\\s${name}="([^"]*)"`).exec(tag)?.[1] ?? null;

/** Every text input's accessible name: its aria-label, else its `<label for>`'s text. */
function inputNames(html: string): string[] {
  const labels = new Map<string, string>();
  for (const m of html.matchAll(/<label[^>]*\sfor="([^"]+)"[^>]*>([\s\S]*?)<\/label>/g)) labels.set(m[1] ?? "", (m[2] ?? "").replace(/<[^>]+>/g, ""));
  return [...html.matchAll(/<input\b[^>]*>/g)].map(([tag]) => attr(tag, "aria-label") ?? labels.get(attr(tag, "id") ?? "") ?? "");
}

describe("the paste dialog's field", () => {
  it("is capped against the window and scrolls inside: the kit's textarea grows with its content", () => {
    expect(PASTE_FIELD_CLASS).toMatch(/(^| )max-h-\[\d+dvh\]( |$)/);
    expect(PASTE_FIELD_CLASS.split(" ")).toContain("overflow-y-auto");
    expect(PROBLEMS_CLASS).toMatch(/(^| )max-h-\[\d+dvh\]( |$)/);
    expect(PROBLEMS_CLASS.split(" ")).toContain("overflow-y-auto");
  });
});

describe("a write's moment", () => {
  it("is shown to the second, so two saves in one minute differ", () => {
    for (const locale of ["en", "ru"]) {
      const a = formatMoment("2026-10-03T12:34:05Z", locale);
      const b = formatMoment("2026-10-03T12:34:56Z", locale);
      expect(a).not.toBe(b);
      expect(b).toMatch(/56/);
    }
  });

  it("falls back to the raw text when it is not a date", () => {
    expect(formatMoment("soon", "en")).toBe("soon");
  });
});

describe("the last write's stamp", () => {
  it("says a model was saved, or that the pricing was taken off", () => {
    expect(pricingStamp(item(cleaning), "en")).toMatchObject({ what: "saved", by: "admin@example.test" });
    expect(pricingStamp(item(null), "en")).toMatchObject({ what: "cleared", by: "admin@example.test" });
    expect(pricingStamp(item(null, null), "en")).toBeNull();
  });

  it("reads 'Taken off … by …' on the status card after a take-off, 'Saved …' after a save", () => {
    const off = render(createElement(PricingStatus, { item: item(null) }));
    expect(off).toMatch(/Taken off [^<]*56[^<]* by admin@example\.test/);
    expect(off).not.toMatch(/Saved /);
    expect(render(createElement(PricingStatus, { item: item(cleaning) }))).toMatch(/Saved [^<]*56[^<]* by admin@example\.test/);
  });

  it("reads 'Taken off' in the conflict alert when the write that won took the pricing off", () => {
    const html = render(createElement(ConflictAlert, { current: item(null), onTakeFresh: () => {}, onOverwrite: () => {} }));
    expect(html).toMatch(/Taken off [^<]*56[^<]* by admin@example\.test\. Your draft is kept\./);
  });
});

describe("what a history row says changed", () => {
  it("names the parts that differ from the change before it", () => {
    const later: PricingModel = { ...cleaning, validFrom: "2026-11-01", minimumCents: cleaning.minimumCents + 100 };
    expect(changedParts(later, cleaning)).toEqual(["validFrom", "general"]);
  });

  it("is empty for the same model saved again, needs read back in another key order included", () => {
    const reordered: PricingModel = { ...cleaning, needs: Object.fromEntries(Object.entries(cleaning.needs).reverse()) };
    expect(changedParts(reordered, cleaning)).toEqual([]);
  });

  it("has nothing to compare for the first save or around a take-off", () => {
    expect(changedParts(cleaning, undefined)).toBeNull();
    expect(changedParts(cleaning, null)).toBeNull();
    expect(changedParts(null, cleaning)).toBeNull();
  });
});

describe("the hint under an answer's effect", () => {
  it("for an amount, shows '0 changes nothing' only on an answer that is 0", () => {
    expect(effectHint("add", "0", false)).toBe("pricing.hint.effect.add");
    expect(effectHint("add", "0.00", true)).toBe("pricing.hint.effect.add");
    expect(effectHint("add", "15", true)).toBeNull();
    expect(effectHint("add", "", true)).toBeNull();
  });

  it("for a multiplier or a discount, explains the scale once, under the first answer", () => {
    expect(effectHint("multiply", "110", true)).toBe("pricing.hint.effect.multiply");
    expect(effectHint("multiply", "110", false)).toBeNull();
    expect(effectHint("discount", "0", false)).toBeNull();
  });
});

describe("the editor's fields to a screen reader", () => {
  it("have one accessible name each across the whole form", () => {
    const html = render(createElement(PricingEditorForm, { editor: editorOf(item(cleaning)), fresher: null, onTakeFresh: () => {}, onReload: () => {} }));
    const names = inputNames(html);
    expect(names.length).toBeGreaterThan(20);
    expect(names.filter((n, i) => names.indexOf(n) !== i)).toEqual([]);
    expect(names).toContain("Studio — Adds, €");
    expect(names).toContain("Studio — Label (FR)");
  });

  it("name a field's hint and its reasons in aria-describedby, by ids that exist", () => {
    const field = FIELD.optionValue("k1");
    const errors: FieldErrors = { byField: new Map([[field, [{ code: "money", vars: {} }, { text: "Too dear." }]]]), general: [], first: field };
    const html = render(createElement(TextField, { field, label: "Adds, €", hint: "0 changes nothing.", value: "x", onChange: () => {}, errors }));
    const ids = [hintIdOf(field), messageIdOf(field, 0), messageIdOf(field, 1)];
    expect(attr(/<input\b[^>]*>/.exec(html)?.[0] ?? "", "aria-describedby")).toBe(ids.join(" "));
    for (const id of ids) expect(html).toContain(`id="${id}"`);
  });

  it("carry no aria-describedby with neither a hint nor a reason", () => {
    expect(describedByOf("roundTo", false, 0)).toBeUndefined();
    const html = render(createElement(TextField, { field: "roundTo", label: "Round", value: "1", onChange: () => {}, errors: NO_FIELD_ERRORS }));
    expect(html).not.toContain("aria-describedby");
  });
});
