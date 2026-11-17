import { describe, expect, it } from "vitest";

import type { Permission } from "@/shared/api/schema";
import type { T } from "@/shared/i18n";
import type { ViewName } from "@/shared/lib/router";
import { sitesNav } from "@/views/nav/model/items";

const t: T = (key) => key;
const OPERATOR: Permission[] = ["sa:sites:work:read", "sa:sites:analysis:read"];

/** Each section as `id: route route…`, the current one starred. */
const shape = (permissions: Permission[], current: ViewName | null = null) =>
  sitesNav(t, permissions, current).map((s) => `${s.id}: ${s.items.map((i) => `${i.view}${i.current ? "*" : ""}`).join(" ")}`);

describe("the Sites dropdown", () => {
  it("has a section per permission its views need: Work, then Analysis with Overview", () => {
    expect(shape(OPERATOR)).toEqual(["sa:sites:work:read: leads places pricing invoices", "sa:sites:analysis:read: overview experiments"]);
    expect(shape(["sa:sites:work:read"])).toEqual(["sa:sites:work:read: leads places pricing invoices"]);
    expect(shape(["sa:sites:analysis:read", "sa:sites:work:pii:see"])).toEqual(["sa:sites:analysis:read: overview experiments"]);
    expect(shape(["sa:sites:work:leads:edit"])).toEqual([]);
  });

  it("links under the host's base with each view's key, and marks the screen that is open", () => {
    expect(sitesNav(t, OPERATOR, null)[0]?.items.map((i) => [i.href, i.key])).toEqual([
      ["/sites/leads", "l"],
      ["/sites/places", "p"],
      ["/sites/pricing", "r"],
      ["/sites/invoices", "i"],
    ]);
    expect(shape(OPERATOR, "experiments")).toEqual(["sa:sites:work:read: leads places pricing invoices", "sa:sites:analysis:read: overview experiments*"]);
  });
});
