export const CSRF_HEADER = "x-sa-csrf";

/** The value of cookie `name` in a `document.cookie` string; null when absent or empty. */
export function readCsrf(cookie: string, name: string): string | null {
  for (const pair of cookie.split(";")) {
    const at = pair.indexOf("=");
    if (at >= 0 && pair.slice(0, at).trim() === name) return pair.slice(at + 1).trim() || null;
  }
  return null;
}
