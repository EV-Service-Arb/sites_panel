import { describe, expect, it } from "vitest";

import { biggestLoss } from "@/entities/funnel/model/funnel";
import type { FunnelStep } from "@/shared/api/schema";
import { translator } from "@/shared/i18n/translate";
import { type Share, formatShare } from "@/shared/lib/share";

const en = translator("en");
const ru = translator("ru");

describe("a share on screen", () => {
  it("is n of m, never a percent, when the backend withheld the percent", () => {
    const small = { n: 12, of: 17, percent: null, small_sample: true };
    expect(formatShare(small, en)).toBe("12 of 17");
    expect(formatShare(small, ru)).toBe("12 из 17");
    expect(formatShare(small, en)).not.toMatch(/%/);
  });

  it("does not compute a percent of its own even if one slipped through with small_sample", () => {
    expect(formatShare({ n: 12, of: 17, percent: 71, small_sample: true }, en)).toBe("12 of 17");
  });

  it("is the backend's whole percent once the sample is large enough", () => {
    expect(formatShare({ n: 10, of: 30, percent: 33, small_sample: false }, en)).toBe("33%");
  });
});

/** A share as the backend would answer it; the percent does not matter to the biggest loss. */
const shareOf = (n: number, of: number): Share => ({ n, of, percent: null, small_sample: true });

describe("the biggest loss", () => {
  const step = (stage: FunnelStep["stage"], reached: number, prev: number | null): FunnelStep => ({
    stage,
    reached,
    of_previous: prev === null ? null : shareOf(reached, prev),
    of_leads: shareOf(reached, 20),
  });

  it("is the step that drops the most leads, counted in leads", () => {
    const steps = [step("created", 20, null), step("contacted", 17, 20), step("quoted", 9, 17), step("won", 5, 9)];
    expect(biggestLoss(steps)).toBe("quoted");
  });

  it("is none on a tie or when nothing is lost", () => {
    expect(biggestLoss([step("created", 10, null), step("contacted", 8, 10), step("quoted", 6, 8)])).toBeNull();
    expect(biggestLoss([step("created", 3, null), step("contacted", 3, 3)])).toBeNull();
  });
});
