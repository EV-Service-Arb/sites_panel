import { describe, expect, it } from "vitest";
import { parseLiveExperiments } from "../src/index.js";

describe("parseLiveExperiments", () => {
  it("takes the panel's answer, passing over fields it added since", () => {
    expect(parseLiveExperiments({ experiments: { a: { holdout: 0.2 }, b: { weights: [0, 1], extra: true } } })).toEqual({ experiments: { a: { holdout: 0.2 }, b: { weights: [0, 1] } } });
    expect(parseLiveExperiments({ experiments: {} })).toEqual({ experiments: {} });
  });

  it("is null for anything that is not the panel's answer", () => {
    for (const body of [null, [], "x", {}, { experiments: null }, { experiments: [] }, { experiments: { a: { enabled: "no" } } }, { experiments: { a: { weights: [1, "2"] } } }, { experiments: { c: null } }, { experiments: { d: 3 } }]) {
      expect(parseLiveExperiments(body)).toBeNull();
    }
  });
});
