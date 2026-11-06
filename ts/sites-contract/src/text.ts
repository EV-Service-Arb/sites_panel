// Text as the panel's Rust reads it, so both sides agree on what is blank, long or one line.

const EDGES = /^\p{White_Space}+|\p{White_Space}+$/gu;

/** Rust's `str::trim`: Unicode White_Space only — not the BOM JavaScript's `trim` also takes. */
export const trim = (s: string): string => s.replace(EDGES, "");

/** In Unicode scalar values, as Rust's `chars().count()`. */
export const charCount = (s: string): number => [...s].length;

export const hasControl = (s: string): boolean => /\p{Cc}/u.test(s);

export const hasWhitespace = (s: string): boolean => /\p{White_Space}/u.test(s);

export type Json = Record<string, unknown>;

export const isObject = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);

export const at = (path: string, key: string | number): string => (typeof key === "number" ? `${path}[${key}]` : path === "" ? key : `${path}.${key}`);

/** `YYYY-MM-DD` of ASCII digits, and a day the (proleptic Gregorian) calendar has. */
export function isDate(v: unknown): v is string {
  if (typeof v !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(v)) return false;
  const [year = 0, month = 0, day = 0] = v.split("-").map(Number);
  const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  return days !== undefined && day >= 1 && day <= days;
}
