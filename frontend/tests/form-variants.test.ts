import { describe, expect, it } from "vitest";

import { quotedPrice } from "@/entities/lead/model/pricing";
import { formatCents, formatDay } from "@/shared/lib/format";

describe("a lead's form variant", () => {
  it("shows a price only for estimate and fixed", () => {
    expect(quotedPrice({ flow: "fixed", quoted_cents: 8_900 })).toBe(8_900);
    expect(quotedPrice({ flow: "quote", quoted_cents: 8_900 })).toBeNull();
    expect(quotedPrice({ flow: null, quoted_cents: null })).toBeNull();
  });
});

describe("money from cents", () => {
  // Intl puts a narrow no-break space in some locales; compared on plain spaces.
  const plain = (s: string) => s.replace(/[  ]/g, " ");

  it("shows whole euros bare and cents only when there are some", () => {
    expect(formatCents(8_900, "EUR", "en")).toBe("€89");
    expect(formatCents(8_950, "EUR", "en")).toBe("€89.50");
    expect(plain(formatCents(123_405, "EUR", "ru"))).toBe("1 234,05 €");
    expect(plain(formatCents(5, "EUR", "fr"))).toBe("0,05 €");
  });

  it("stays exact past what a float holds", () => {
    expect(formatCents(900_719_925_474_099, "EUR", "en")).toBe("€9,007,199,254,740.99");
  });
});

describe("the price model's day", () => {
  it("shows as that day in every zone", () => {
    expect(formatDay("2026-09-28", "en")).toBe("Sep 28, 2026");
  });
});
