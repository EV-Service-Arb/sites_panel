import { PRICING_CURRENCY, type PricingInputKind } from "@evinvest/sites-contract";

import { formatCents, parseMoney } from "@/shared/lib/format";

/**
 * The model keeps hundredths everywhere: cents of a euro, basis points of a
 * percent. The editor types them as decimals with at most two places — "45",
 * "45.50", "110", "12.5" — and never through a float.
 */
export function hundredthsText(n: number): string {
  const whole = Math.floor(n / 100);
  const rest = n % 100;
  return rest === 0 ? String(whole) : `${whole}.${String(rest).padStart(2, "0")}`;
}

/** "45", "45.5", "45,50" → 4550; anything else (a sign, three decimals, words) → null. */
export const parseHundredths: (raw: string) => number | null = parseMoney;

/** An option's effect as the reader takes it in: "+15 €", "×110 %", "−15 %". */
export function effectText(kind: PricingInputKind, value: number, locale: string): string {
  switch (kind) {
    case "add":
      return `+${formatCents(value, PRICING_CURRENCY, locale)}`;
    case "multiply":
      return `×${hundredthsText(value)} %`;
    case "discount":
      return `−${hundredthsText(value)} %`;
  }
}
