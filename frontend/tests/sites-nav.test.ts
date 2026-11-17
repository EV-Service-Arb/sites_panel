import { describe, expect, it } from "vitest";

import type { Permission } from "@/shared/api/schema";
import type { T } from "@/shared/i18n";
import type { Route } from "@/shared/lib/router";
import { sitesNav } from "@/views/nav/model/items";

const t: T = (key) => key;
const OPERATOR: Permission[] = ["sa:sites:work:read", "sa:sites:analysis:read"];

/** Each section as `id: route route…`, the current one starred. */
const shape = (permissions: Permission[], current: Route | null = null) =>
  sitesNav(t, permissions, "/sites", current).map((s) => `${s.id}: ${s.items.map((i) => `${i.route}${i.current ? "*" : ""}`).join(" ")}`);

describe("the Sites dropdown", () => {
  it("has a section per permission that opens it: Work, then Analysis with Overview", () => {
    expect(shape(OPERATOR)).toEqual(["work: leads places pricing invoices", "analysis: overview experiments"]);
    expect(shape(["sa:sites:work:read"])).toEqual(["work: leads places pricing invoices"]);
    expect(shape(["sa:sites:analysis:read", "sa:sites:work:pii:see"])).toEqual(["analysis: overview experiments"]);
    expect(shape(["sa:sites:work:leads:edit"])).toEqual([]);
  });

  it("links under the host's base and marks the screen that is open", () => {
    expect(sitesNav(t, OPERATOR, "/sites", null)[0]?.items[0]?.href).toBe("/sites/leads");
    expect(shape(OPERATOR, "experiments")).toEqual(["work: leads places pricing invoices", "analysis: overview experiments*"]);
  });
});
