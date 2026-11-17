import { useEffect, useRef } from "react";

/** Reloads when `version` moves past the one it mounted at: one's own write shows at once, socket or not. */
export function useRereadOn(version: number, reload: () => void): void {
  const seen = useRef(version);
  useEffect(() => {
    if (seen.current === version) return;
    seen.current = version;
    reload();
  }, [version, reload]);
}
