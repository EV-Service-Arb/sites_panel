import { describe, expect, it } from "vitest";

import type { Permission } from "@/shared/api";
import type { T } from "@/shared/i18n";
import type { Route } from "@/shared/lib/router";
import { type Badge, sitesNav } from "@/views/nav/model/items";
import type { Marks } from "@/views/nav/model/use-marks";

const t: T = (key) => key;
const QUIET: Marks = { leads: 0, away: 0, places: 0, experiments: false, bookings: false };
const OPERATOR: Permission[] = ["sa:sites:work:read", "sa:sites:analysis:read"];

/** Each section as `id: route route…`, the current one starred. */
const shape = (permissions: Permission[], current: Route | null = null) =>
  sitesNav(t, permissions, QUIET, "/sites", current).map((s) => `${s.id}: ${s.items.map((i) => `${i.route}${i.current ? "*" : ""}`).join(" ")}`);

const badges = (marks: Partial<Marks>): Record<string, Badge> =>
  Object.fromEntries(sitesNav(t, OPERATOR, { ...QUIET, ...marks }, "/sites", null).flatMap((s) => s.items.map((i) => [i.route, i.badge])));

describe("the Sites dropdown", () => {
  it("has a section per permission that opens it: Work, then Analysis with Overview", () => {
    expect(shape(OPERATOR)).toEqual(["work: leads places pricing invoices", "analysis: overview experiments"]);
    expect(shape(["sa:sites:work:read"])).toEqual(["work: leads places pricing invoices"]);
    expect(shape(["sa:sites:analysis:read", "sa:sites:work:pii:see"])).toEqual(["analysis: overview experiments"]);
    expect(shape(["sa:sites:work:leads:edit"])).toEqual([]);
  });

  it("links under the host's base and marks the screen that is open", () => {
    expect(sitesNav(t, OPERATOR, QUIET, "/sites", null)[0]?.items[0]?.href).toBe("/sites/leads");
    expect(shape(OPERATOR, "experiments")).toEqual(["work: leads places pricing invoices", "analysis: overview experiments*"]);
  });

  it("counts new leads and changed places, and dots experiments", () => {
    expect(badges({})).toEqual({ leads: null, places: null, pricing: null, invoices: null, overview: null, experiments: null });
    expect(badges({ leads: 3, places: 2, experiments: true })).toMatchObject({
      leads: { kind: "count", count: 3 },
      places: { kind: "count", count: 2 },
      experiments: { kind: "dot", label: "nav.changed" },
    });
  });

  it("dots Leads for bookings without a lead only while no new lead is counted", () => {
    expect(badges({ bookings: true }).leads).toEqual({ kind: "dot", label: "nav.unmatched" });
    expect(badges({ bookings: true, leads: 1 }).leads).toEqual({ kind: "count", count: 1 });
  });
});
