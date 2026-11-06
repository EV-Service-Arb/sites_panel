import { readFileSync, readdirSync } from "node:fs";

import type { PricingModel } from "@evinvest/sites-contract";
import { describe, expect, it } from "vitest";

import { type PricingDraft, draftOf, emptyInput, emptyNeed, emptyOption, fingerprint } from "@/features/edit-pricing/model/draft";
import { errorsOf } from "@/features/edit-pricing/model/errors";
import { FIELD } from "@/features/edit-pricing/model/fields";
import { exportModel, importModel } from "@/features/edit-pricing/model/import";
import { addInput, moveNeedInput, removeInput, setInputKind, toggleNeedInput } from "@/features/edit-pricing/model/ops";
import { fieldOfPath, segmentsOf } from "@/features/edit-pricing/model/paths";
import { modelOf } from "@/features/edit-pricing/model/serialize";

const FIXTURES = new URL("../../contracts/vectors/pricing/", import.meta.url);
const read = (dir: string, name: string): unknown => JSON.parse(readFileSync(new URL(`${dir}/${name}`, FIXTURES), "utf8"));
const names = (dir: string) => readdirSync(new URL(`${dir}/`, FIXTURES)).filter((n) => n.endsWith(".json"));
const cleaning = read("valid", "cleaning.json") as PricingModel;
const LOCALES = ["fr", "en"];
const TODAY = "2026-10-03";

describe("the draft serialised", () => {
  it("gives back the model it was read from, exactly: same keys, integers", () => {
    const { model, problems } = modelOf(draftOf(cleaning, TODAY), LOCALES);
    expect(problems).toEqual([]);
    expect(model).toEqual(cleaning);
    // Key by key, as the server and the site refuse a field they do not know.
    expect(JSON.parse(exportModel(model as PricingModel))).toStrictEqual(cleaning);
    for (const fixture of names("valid")) {
      const m = read("valid", fixture) as PricingModel;
      const locales = [...new Set(m.inputs.flatMap((i) => Object.keys(i.labels)))];
      expect(modelOf(draftOf(m, TODAY), locales).model, fixture).toStrictEqual(m);
    }
  });

  it("turns euros into cents and percents into basis points, never floats", () => {
    const draft = draftOf(cleaning, TODAY);
    const [zone, bedrooms, , frequency] = draft.inputs;
    if (!zone || !bedrooms || !frequency) throw new Error("fixture changed");
    zone.options[1] = { ...zone.options[1]!, value: "112.5" };
    bedrooms.options[1] = { ...bedrooms.options[1]!, value: "15,10" };
    frequency.options[0] = { ...frequency.options[0]!, value: "12.34" };
    draft.minimum = "49.9";
    const model = modelOf(draft, LOCALES).model;
    expect(model?.minimumCents).toBe(4990);
    expect(model?.inputs[0]?.options[1]).toEqual({ id: "proche", labels: { fr: "Proche banlieue", en: "Inner suburbs" }, multiplyBp: 11250 });
    expect(model?.inputs[1]?.options[1]).toEqual({ id: "t2", labels: { fr: "1 chambre", en: "1 bedroom" }, addCents: 1510 });
    expect(model?.inputs[3]?.options[0]).toMatchObject({ discountBp: 1234 });
  });

  it("carries only the model's fields: no keys, no blank labels, no amount of the other kind", () => {
    let draft = addInput(draftOf(null, TODAY));
    const input = draft.inputs[0]!;
    draft = { ...draft, roundTo: "1", inputs: [{ ...input, id: "zone", labels: { fr: "Zone", en: "Area", de: " " }, options: [{ ...input.options[0]!, id: "a", labels: { fr: "A", en: "A" } }] }] };
    draft = setInputKind(draft, input.key, "multiply");
    draft = { ...draft, needs: [{ ...emptyNeed(), id: "std", amount: "45", inputs: [input.key] }] };
    const { model, problems } = modelOf(draft, LOCALES);
    expect(problems).toEqual([]);
    expect(model).toStrictEqual({
      format: 1,
      currency: "EUR",
      validFrom: TODAY,
      roundToCents: 100,
      minimumCents: 0,
      inputs: [{ id: "zone", kind: "multiply", labels: { fr: "Zone", en: "Area" }, options: [{ id: "a", labels: { fr: "A", en: "A" }, multiplyBp: 10000 }] }],
      needs: { std: { kind: "estimate", baseCents: 4500, inputs: ["zone"] } },
    });
  });

  it("keeps a need's questions through a rename, and drops a removed one", () => {
    let draft = draftOf(cleaning, TODAY);
    const zone = draft.inputs[0]!;
    draft = { ...draft, inputs: draft.inputs.map((i) => (i.key === zone.key ? { ...i, id: "area" } : i)) };
    expect(modelOf(draft, LOCALES).model?.needs.standard).toEqual({ kind: "estimate", baseCents: 4500, inputs: ["area", "bedrooms", "surface", "frequency"] });
    const need = draft.needs[0]!;
    draft = moveNeedInput(draft, need.key, zone.key, 1);
    expect(modelOf(draft, LOCALES).model?.needs.standard).toMatchObject({ inputs: ["bedrooms", "area", "surface", "frequency"] });
    draft = removeInput(draft, zone.key);
    expect(modelOf(draft, LOCALES).model?.needs.standard).toMatchObject({ inputs: ["bedrooms", "surface", "frequency"] });
  });

  it("is unchanged by a round trip, and changed by an edit", () => {
    const draft = draftOf(cleaning, TODAY);
    expect(fingerprint(draft)).toBe(fingerprint(draftOf(cleaning, TODAY)));
    expect(fingerprint({ ...draft, minimum: "50" })).not.toBe(fingerprint(draft));
    expect(fingerprint(draftOf(null, TODAY))).toBe(fingerprint(draftOf(null, TODAY)));
  });
});

const problemsAt = (draft: PricingDraft, locales = LOCALES) => modelOf(draft, locales).problems.map((p) => [p.field, p.code]);

describe("the editor's own checks", () => {
  it("says what is wrong at the field, in its units", () => {
    const draft = draftOf(cleaning, TODAY);
    const [zone, bedrooms, , frequency] = draft.inputs;
    if (!zone || !bedrooms || !frequency) throw new Error("fixture changed");
    zone.id = "Zone 1";
    bedrooms.options[0] = { ...bedrooms.options[0]!, value: "12.345" };
    zone.options[0] = { ...zone.options[0]!, value: "x" };
    frequency.options[0] = { ...frequency.options[0]!, value: "150" };
    draft.validFrom = "2026-02-30";
    const at = problemsAt(draft);
    expect(at).toContainEqual([FIELD.inputId(zone.key), "slug"]);
    expect(at).toContainEqual([FIELD.optionValue(bedrooms.options[0]!.key), "money"]);
    expect(at).toContainEqual([FIELD.optionValue(zone.options[0]!.key), "percent"]);
    expect(at).toContainEqual([FIELD.optionValue(frequency.options[0]!.key), "int"]);
    expect(at).toContainEqual([FIELD.validFrom, "date"]);
  });

  it("wants a label in each of the brand's locales, and only those", () => {
    const draft = draftOf(cleaning, TODAY);
    const zone = draft.inputs[0]!;
    zone.labels = { fr: "Zone", en: "  " };
    expect(problemsAt(draft)).toEqual([[FIELD.inputLabel(zone.key, "en"), "labelMissing"]]);
    expect(problemsAt(draft, ["fr"])).toEqual([]);
    // A label missing does not stop a preview, only a save.
    expect(modelOf(draft, LOCALES).model).not.toBeNull();
  });

  it("refuses a need twice, and one asking more than twelve questions", () => {
    let draft = draftOf(null, TODAY);
    for (let i = 0; i < 13; i += 1) draft = addInput(draft);
    draft = {
      ...draft,
      inputs: draft.inputs.map((input, i) => ({ ...input, id: `q${i}`, labels: { fr: "Q", en: "Q" }, options: [{ ...emptyOption("add"), id: "a", labels: { fr: "A", en: "A" } }] })),
    };
    const need = { ...emptyNeed(), id: "std", amount: "10" };
    draft = { ...draft, needs: [need, { ...emptyNeed(), id: "std", amount: "20" }] };
    for (const input of draft.inputs) draft = toggleNeedInput(draft, need.key, input.key);
    const at = problemsAt(draft);
    expect(at).toContainEqual([FIELD.needId(draft.needs[1]!.key), "duplicate"]);
    expect(at).toContainEqual([FIELD.needInputs(need.key), "list"]);
  });

  it("keeps a new, empty row quiet until a save is tried", () => {
    const draft = { ...draftOf(null, TODAY), inputs: [emptyInput()] };
    const { problems } = modelOf(draft, LOCALES);
    const field = FIELD.inputId(draft.inputs[0]!.key);
    expect(problems.map((p) => [p.field, p.code])).toContainEqual([field, "required"]);
    expect(errorsOf(problems, false, null).byField.has(field)).toBe(false);
    expect(errorsOf(problems, true, null).byField.get(field)).toEqual([{ code: "required", vars: {} }]);
    expect(errorsOf(problems, true, null).first).toBe(field);
  });

  it("does not say twice what the draft already faults", () => {
    const draft = { ...draftOf(null, TODAY), inputs: [emptyInput()] };
    const fields = modelOf(draft, LOCALES).problems.map((p) => p.field);
    // The contract's "a slug" and "at least one label" are covered by "required" and "label in fr/en".
    expect(fields.filter((f) => f === FIELD.inputId(draft.inputs[0]!.key))).toHaveLength(1);
    expect(fields).not.toContain(FIELD.inputLabels(draft.inputs[0]!.key));
  });
});

describe("a server path, onto the editor's field", () => {
  const draft = draftOf(cleaning, TODAY);
  const [zone, bedrooms] = draft.inputs;
  const need = draft.needs[0]!;
  if (!zone || !bedrooms) throw new Error("fixture changed");

  it("reads index and slug segments alike", () => {
    expect(segmentsOf("inputs[2].options.studio.labels.en")).toEqual(["inputs", 2, "options", "studio", "labels", "en"]);
    expect(segmentsOf("needs.standard.inputs[2]")).toEqual(["needs", "standard", "inputs", 2]);
  });

  it.each([
    ["inputs.bedrooms.options.studio.labels.en", FIELD.optionLabel(bedrooms.options[0]!.key, "en")],
    ["inputs[1].options[0].addCents", FIELD.optionValue(bedrooms.options[0]!.key)],
    ["inputs.zone.labels", FIELD.inputLabels(zone.key)],
    ["inputs[0].kind", FIELD.inputKind(zone.key)],
    ["inputs.zone.options[9]", FIELD.inputOptions(zone.key)],
    ["needs.standard.inputs[2]", FIELD.needInputs(need.key)],
    ["needs.standard.baseCents", FIELD.needAmount(need.key)],
    ["needs.standard", FIELD.need(need.key)],
    ["needs.gone", FIELD.needs],
    ["inputs.gone.labels.en", FIELD.inputs],
    ["minimumCents", FIELD.minimum],
  ])("%s", (path, field) => {
    expect(fieldOfPath(path, draft)).toBe(field);
  });

  it("leaves what no field holds for the top of the form", () => {
    expect(fieldOfPath("format", draft)).toBeNull();
    expect(fieldOfPath("", draft)).toBeNull();
    const errors = errorsOf([], false, { field: null, path: "currency", message: "EUR" });
    expect(errors.general).toEqual([{ text: "currency: EUR" }]);
  });
});

describe("a pasted model", () => {
  it("comes in whole when the contract takes it", () => {
    expect(importModel(JSON.stringify(cleaning))).toEqual({ kind: "ok", model: cleaning });
  });

  it("is refused for a field the model does not have, or for not being JSON", () => {
    const refused = importModel(JSON.stringify({ ...cleaning, note: "x" }));
    expect(refused.kind).toBe("invalid");
    expect(refused.kind === "invalid" && refused.problems[0]).toMatchObject({ path: "note", code: "unknownField" });
    expect(importModel("{ format: 1 ")).toEqual({ kind: "not_json" });
  });
});
