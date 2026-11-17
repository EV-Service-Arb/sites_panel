import type { Stage } from "@/entities/lead";
import type { Currency, Payment } from "@/shared/api/schema";
import { parseMoney } from "@/shared/lib/format";

/** A job has to be won before money for it comes in. */
export function takesPayment(stage: Stage): boolean {
  return stage === "won" || stage === "completed" || stage === "paid";
}

/** The form's text → the payment the API takes, or null while it would be refused (`0 ≤ commission ≤ billed`). */
export function paymentFrom(billed: string, commission: string, currency: Currency): Payment | null {
  const b = parseMoney(billed);
  const c = parseMoney(commission);
  if (b === null || c === null || c > b) return null;
  return { billed: b, commission: c, currency };
}
