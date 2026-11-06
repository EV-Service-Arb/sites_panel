import { describe, expect, it } from "vitest";

import { type Experiment, experimentsParser, sharesOf, statusOf } from "@/entities/experiment";
import { RESET_PATCH, checkDraft, draftOf, enabledPatch, parseDecimal, resetMovesWeights } from "@/features/configure-experiment/model/draft";
import { parseLiveMessage } from "@/shared/lib/live/protocol";
import { ParseError, parse } from "@/shared/lib/parse";
import { byBrand } from "@/views/experiments/model/group";

const item = {
  brand: "aquafix",
  key: "lead_layout",
  variants: ["a", "b"],
  declared: { weights: [1, 1], enabled: true, holdout: null, summary: "Shorter form converts better", declared_at: "2026-10-01T09:00:00Z" },
  override: null,
  effective: { weights: [1, 1], enabled: true, holdout: null },
  weights_changed_at: null,
  retired: false,
  posthog_url: "https://us.posthog.com/project/1/insights/new#q=%7B%7D",
};

const read = (over: Record<string, unknown> = {}): Experiment => {
  const e = parse(experimentsParser, { experiments: [{ ...item, ...over }] }).experiments[0];
  if (!e) throw new Error("no experiment");
  return e;
};

describe("the experiments answer", () => {
  it("is read as config: variants, declaration, override, effective split", () => {
    const e = read({ override: { weights: [3, 1], enabled: null, holdout: null, changed_by: "admin@example.test", changed_at: "2026-10-02T10:00:00Z" }, effective: { weights: [3, 1], enabled: true, holdout: null } });
    expect(e.variants).toEqual(["a", "b"]);
    expect(e.override?.weights).toEqual([3, 1]);
    expect(e.override?.enabled).toBeNull();
    expect(e.declared.summary).toBe("Shorter form converts better");
  });

  it("takes an absent posthog_url as none and refuses one that is not http(s)", () => {
    expect(read({ posthog_url: null }).posthog_url).toBeNull();
    expect(() => read({ posthog_url: "javascript:alert(1)" })).toThrow(ParseError);
    expect(() => read({ posthog_url: "not a url" })).toThrow(ParseError);
  });
});

describe("a row", () => {
  it("shows the split in percent", () => {
    expect(sharesOf([3, 1])).toEqual([75, 25]);
    expect(sharesOf([0, 0])).toEqual([0, 0]);
  });

  it("says retired before off, and holdout only while running", () => {
    expect(statusOf(read({ retired: true, effective: { weights: [1, 1], enabled: false, holdout: null } }))).toBe("retired");
    expect(statusOf(read({ effective: { weights: [1, 1], enabled: false, holdout: 0.1 } }))).toBe("off");
    expect(statusOf(read({ effective: { weights: [1, 1], enabled: true, holdout: 0.1 } }))).toBe("holdout");
    expect(statusOf(read({ effective: { weights: [1, 1], enabled: true, holdout: 0 } }))).toBe("running");
  });

  it("groups by brand, brands in order, the retired last", () => {
    const groups = byBrand([read({ brand: "vifnet" }), read({ key: "old", retired: true }), read({ key: "new" })]);
    expect(groups.map(([b, xs]) => [b, xs.map((x) => x.key)])).toEqual([["aquafix", ["new", "old"]], ["vifnet", ["lead_layout"]]]);
  });
});

describe("a draft", () => {
  const e = read();

  it("reads decimals with a point or a comma, and nothing else", () => {
    expect(parseDecimal("0,5")).toBe(0.5);
    expect(parseDecimal(" 2 ")).toBe(2);
    expect(parseDecimal("-1")).toBeNull();
    expect(parseDecimal("1e3")).toBeNull();
    expect(parseDecimal("")).toBeNull();
  });

  it("starts from the effective split, a holdout in percent", () => {
    expect(draftOf(read({ effective: { weights: [3, 1], enabled: true, holdout: 0.07 } }))).toEqual({ weights: ["3", "1"], holdout: "7" });
  });

  it("is refused before sending when a landing would ignore it", () => {
    expect(checkDraft(e, { weights: ["0", "0"], holdout: "" })).toEqual({ kind: "invalid", errors: { weights: true } });
    expect(checkDraft(e, { weights: ["1", "x"], holdout: "100" })).toEqual({ kind: "invalid", errors: { weights: true, holdout: true } });
  });

  it("sends nothing when nothing changed", () => {
    expect(checkDraft(e, { weights: ["1", "1"], holdout: "0" }).kind).toBe("unchanged");
  });

  it("sends only what changed, and warns when the split moves", () => {
    expect(checkDraft(e, { weights: ["3", "1"], holdout: "" })).toEqual({ kind: "ready", patch: { weights: [3, 1] }, weightsChanged: true });
    expect(checkDraft(e, { weights: ["1", "1"], holdout: "10" })).toEqual({ kind: "ready", patch: { holdout: 0.1 }, weightsChanged: false });
  });

  it("sends null for a value equal to the code's, so the override holds only deviations", () => {
    const moved = read({ effective: { weights: [3, 1], enabled: true, holdout: 0.2 } });
    expect(checkDraft(moved, { weights: ["1", "1"], holdout: "" })).toEqual({ kind: "ready", patch: { weights: null, holdout: null }, weightsChanged: true });
  });

  it("sends a holdout of 0, not null, to take out one the code declares", () => {
    const declared = read({ declared: { ...item.declared, holdout: 0.1 }, effective: { weights: [1, 1], enabled: true, holdout: 0.1 } });
    expect(checkDraft(declared, { weights: ["1", "1"], holdout: "" })).toEqual({ kind: "ready", patch: { holdout: 0 }, weightsChanged: false });
  });

  it("switches back to the code's value as null", () => {
    expect(enabledPatch(e, false)).toEqual({ enabled: false });
    expect(enabledPatch(e, true)).toEqual({ enabled: null });
  });

  it("resets every field, and knows when that moves traffic", () => {
    expect(RESET_PATCH).toEqual({ enabled: null, weights: null, holdout: null });
    expect(resetMovesWeights(read({ effective: { weights: [3, 1], enabled: true, holdout: null } }))).toBe(true);
    expect(resetMovesWeights(read({ effective: { weights: [1, 1], enabled: false, holdout: null } }))).toBe(false);
  });
});

describe("the live socket", () => {
  it("no longer knows the metrics topic: such a message is ignored", () => {
    expect(parseLiveMessage({ type: "changed", topic: "metrics", brand_id: null, id: null, at: "2026-10-04T00:00:00Z" })).toBeNull();
    expect(parseLiveMessage({ type: "changed", topic: "experiments", brand_id: "aquafix", id: null, at: "2026-10-04T00:00:00Z" })).not.toBeNull();
  });
});
