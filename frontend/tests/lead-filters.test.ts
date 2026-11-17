import { describe, expect, it } from "vitest";

import { segmentLabels } from "@/features/lead-filters/model/filter";
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
