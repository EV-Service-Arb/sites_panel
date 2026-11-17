import openapi from "@/shared/api/openapi.json";

// The bounds the document holds an invoice to. The form checks them so that the operator sees the
// problem before the request; the backend checks them again.
const { amount, memo, expires_in_days } = openapi.components.schemas.NewInvoice.properties;
const AMOUNT = new RegExp(amount.pattern);
/** Banking counts the memo in characters (code points), not in bytes or UTF-16 units. */
export const MAX_MEMO_CHARS = memo.maxLength;
export const MIN_EXPIRES_IN_DAYS = expires_in_days.minimum;
export const MAX_EXPIRES_IN_DAYS = expires_in_days.maximum;

/**
 * `raw` as banking's canonical decimal (`12.5`, `0.5`, `1000`), or null when the backend would
 * refuse it. One comma reads as the decimal point: a Russian keyboard types `12,5`, and the
 * backend takes only a point.
 */
export function canonicalAmount(raw: string): string | null {
  const text = raw.trim().replace(",", ".");
  if (!/^\d*(\.\d*)?$/.test(text)) return null;
  const [whole = "", fraction = ""] = text.split(".");
  const int = whole.replace(/^0+/, "");
  const frac = fraction.replace(/0+$/, "");
  const canonical = `${int === "" ? "0" : int}${frac === "" ? "" : `.${frac}`}`;
  return AMOUNT.test(canonical) ? canonical : null;
}

/** The memo the payer reads: within the length, on one line, without control characters. */
export function memoProblem(memo: string): "too_long" | "control" | null {
  if ([...memo].length > MAX_MEMO_CHARS) return "too_long";
  if (/[\u0000-\u001F\u007F-\u009F]/.test(memo)) return "control";
  return null;
}

/** Whole days within the document's range, or null. */
export function expiresInDays(raw: string): number | null {
  const text = raw.trim();
  if (!/^\d{1,2}$/.test(text)) return null;
  const days = Number(text);
  return days >= MIN_EXPIRES_IN_DAYS && days <= MAX_EXPIRES_IN_DAYS ? days : null;
}
