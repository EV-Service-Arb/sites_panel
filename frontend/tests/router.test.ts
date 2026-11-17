import { describe, expect, it } from "vitest";

import { leadsWith, narrows } from "@/features/lead-filters/model/filter";
import { type Place, type ViewParams, hrefOf, placeOf } from "@/shared/lib/router";

const place = (href: string, base = "/sites"): Place => {
  const url = new URL(href, "http://sites.test");
  return placeOf(url.pathname, url.search, base);
};
const viewOf = (href: string) => {
  const p = place(href);
  return p.kind === "view" ? p.at : p.kind;
};

describe("where an href is under the host's base", () => {
  it("is the start at the base itself, with or without its slash", () => {
    expect(viewOf("/sites")).toEqual({ view: "start", params: {} });
    expect(viewOf("/sites/")).toEqual({ view: "start", params: {} });
  });

  it("is a view under it, a trailing slash ignored, under any base", () => {
    expect(viewOf("/sites/leads")).toEqual({ view: "leads", params: {} });
    expect(viewOf("/sites/experiments/")).toEqual({ view: "experiments", params: {} });
    expect(place("/panel/sites/pricing?brand=vifnet", "/panel/sites")).toEqual({ kind: "view", at: { view: "pricing", params: { brand: "vifnet" } } });
  });

  it("is nothing here for another path, or a key or word no view takes", () => {
    for (const href of ["/sites/sources", "/sites/leads/extra", "/sites/Leads", "/sites/leads?stage=nope", "/sites/leads?overdue=1", "/sites/leads?lead=aquafix/L-1", "/sites/overview?period=14"])
      expect(viewOf(href), href).toBe("unknown");
  });

  it("is outside for a path that only starts like the base", () => {
    for (const href of ["/sitesx/leads", "/review_archive", "/"]) expect(viewOf(href), href).toBe("outside");
  });
});

describe("the leads view's filter", () => {
  const roundTrip = (params: ViewParams<"leads">) => viewOf(hrefOf({ view: "leads", params }));

  it("goes into the URL and back out of it, the open lead with it", () => {
    const params = leadsWith({ stage: "created" }, { booking: "no_show", channel: "telegram", overdue: true, created_from: "2026-09-01", lead: ["aquafix", "L-1"] });
    expect(roundTrip(params)).toEqual({ view: "leads", params });
    expect(hrefOf({ view: "leads", params: { overdue: true } })).toBe("/sites/leads?overdue=");
  });

  it("leaves the URL by a key set to nothing", () => {
    expect(leadsWith({ stage: "created", booking: "booked", overdue: true, message_ref: "AQ-7K3F" }, { booking: null, overdue: false, message_ref: "" })).toEqual({ stage: "created" });
  });

  it("narrows the list by any key, or by the keys asked", () => {
    expect(narrows({})).toBe(false);
    expect(narrows({ channel: "whatsapp" })).toBe(true);
    expect(narrows({ overdue: false })).toBe(false);
    expect(narrows({ channel: "whatsapp" }, ["message_ref"])).toBe(false);
  });
});
