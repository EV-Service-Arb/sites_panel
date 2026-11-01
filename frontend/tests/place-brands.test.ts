import { describe, expect, it } from "vitest";

import { offeredBrands } from "@/views/places/model/brands";

describe("offeredBrands", () => {
  it("offers a brand sites has a price list for before it has any place", () => {
    expect(offeredBrands([{ brand: "aquafix" }], [{ brand_id: "aquafix" }, { brand_id: "vifnet" }])).toEqual(["aquafix", "vifnet"]);
  });

  it("keeps every place's brand once, sorted, with no price lists", () => {
    expect(offeredBrands([{ brand: "vifnet" }, { brand: "aquafix" }, { brand: "vifnet" }], [])).toEqual(["aquafix", "vifnet"]);
  });
});
