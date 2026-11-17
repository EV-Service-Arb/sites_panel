/**
 * One write's Idempotency-Key (the backend takes 1–128 visible ASCII). A key
 * stays with its attempt: retrying the same body after a lost answer sends the
 * same key, so the server replays its first answer instead of acting twice.
 */
export interface Attempt {
  body: string;
  key: string;
}

export function newIdempotencyKey(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") return crypto.randomUUID();
  // Outside a secure context randomUUID is missing; 16 random bytes say as much.
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** The attempt for `body`: the previous one while the body is the same, a new key once it differs. */
export function attemptFor(previous: Attempt | null, body: unknown, mint: () => string = newIdempotencyKey): Attempt {
  const text = JSON.stringify(body);
  return previous?.body === text ? previous : { body: text, key: mint() };
}
