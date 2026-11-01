import { describe, expect, it } from "vitest";

import { type Place, placeOf } from "@/shared/lib/router";

const check = (pathname: string, expected: Place, base = "/sites") => expect(placeOf(pathname, base)).toEqual(expected);

describe("where a path is under the host's base", () => {
  it("is the start at the base itself, with or without its slash", () => {
    check("/sites", { kind: "start" });
    check("/sites/", { kind: "start" });
  });

  it("is a route one segment under it, a trailing slash ignored", () => {
    check("/sites/leads", { kind: "route", route: "leads" });
    check("/sites/experiments/", { kind: "route", route: "experiments" });
    check("/panel/sites/pricing", { kind: "route", route: "pricing" }, "/panel/sites");
  });

  it("is unknown for any other path under it", () => {
    check("/sites/sources", { kind: "unknown" });
    check("/sites/leads/extra", { kind: "unknown" });
    check("/sites/Leads", { kind: "unknown" });
  });

  it("is outside for a path that only starts like the base", () => {
    check("/sitesx/leads", { kind: "outside" });
    check("/review_archive", { kind: "outside" });
    check("/", { kind: "outside" });
  });
});
