import { useState } from "react";

/**
 * The value, or while it is null the last value it had. An overlay closes by
 * animating out for a few hundred milliseconds after its `open` turns false; a
 * body rendered from the live value would empty in the first of those frames
 * and the panel would collapse as it leaves. Compare by identity: pass a
 * string or a state-held object, not one rebuilt on every render.
 */
export function useLastNonNull<T>(value: T | null): T | null {
  const [last, setLast] = useState<T | null>(value);
  if (value !== null && !Object.is(value, last)) setLast(value);
  return value ?? last;
}
