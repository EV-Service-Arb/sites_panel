import { describe, expect, it } from "vitest";

import { leadFilterFrom, paramsWith, segmentLabels } from "@/features/lead-filters/model/params";
import type { LeadCounts } from "@/shared/api/schema";
import { translator } from "@/shared/i18n/translate";

const en = translator("en");
const ru = translator("ru");

const counts: LeadCounts = {
  stages: { created: 3, contacted: 1, quoted: 2, won: 4, completed: 0, paid: 7, lost: 5 },
  overdue: 2,
  total: 22,
};

describe("the stage segments", () => {
  it("carry the count of the stage each lists", () => {
    expect(segmentLabels(counts, ru).map((s) => s.label)).toEqual(["Новые · 3", "В работе · 1", "Сметы · 2"]);
    expect(segmentLabels(counts, en).map((s) => s.stage)).toEqual(["created", "contacted", "quoted"]);
  });

  it("show a zero as a zero, and the bare name until the counts arrive", () => {
    const none = { ...counts, stages: { ...counts.stages, created: 0 } };
    expect(segmentLabels(none, en)[0]?.label).toBe("New · 0");
    expect(segmentLabels(null, en).map((s) => s.label)).toEqual(["New", "In progress", "Quotes"]);
  });
});

describe("the created range in the URL", () => {
  it("round-trips UTC days and drops anything else", () => {
    const params = paramsWith(new URLSearchParams(), { created_from: "2026-09-01", created_to: "2026-09-30" });
    expect(params.toString()).toBe("created_from=2026-09-01&created_to=2026-09-30");
    expect(leadFilterFrom(params)).toMatchObject({ created_from: "2026-09-01", created_to: "2026-09-30" });
    expect(leadFilterFrom(new URLSearchParams("created_from=yesterday")).created_from).toBeNull();
  });
});

describe("the suspect filter", () => {
  it("takes only the words the API takes, and is every lead otherwise", () => {
    expect(leadFilterFrom(new URLSearchParams("suspect=only")).suspect).toBe("only");
    expect(leadFilterFrom(new URLSearchParams("suspect=exclude")).suspect).toBe("exclude");
    expect(leadFilterFrom(new URLSearchParams("suspect=maybe")).suspect).toBeNull();
    expect(leadFilterFrom(new URLSearchParams("")).suspect).toBeNull();
  });

  it("goes into the URL and back out of it", () => {
    const on = paramsWith(new URLSearchParams("stage=created"), { suspect: "exclude" });
    expect(on.toString()).toBe("stage=created&suspect=exclude");
    expect(paramsWith(on, { suspect: null }).toString()).toBe("stage=created");
  });
});
