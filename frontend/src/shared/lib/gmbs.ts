import { useSyncExternalStore } from "react";

import { GMBS_EVENT, tellHost } from "@/shared/lib/host-events";

/** The GMBs the host's outline selects: `null` narrows nothing; `undefined` until the host has said. */
export type Gmbs = readonly number[] | null;

let selected: Gmbs | undefined;
const listeners = new Set<() => void>();
let installed = false;

/** The host's event, read as untrusted: `{ ids: number[] | null }`. */
function idsOf(detail: unknown): Gmbs {
  const ids = typeof detail === "object" && detail !== null && "ids" in detail ? detail.ids : undefined;
  if (ids === null) return null;
  if (Array.isArray(ids) && ids.every((id) => Number.isSafeInteger(id))) return ids as number[];
  throw new Error(`sa:gmbs: ${JSON.stringify(detail)} is not { ids: number[] | null }`);
}

/** Called by each element as it connects: the window listener once per page, the selection asked for each time. */
export function followHostGmbs(): void {
  if (!installed) {
    installed = true;
    window.addEventListener(GMBS_EVENT, (e) => {
      selected = idsOf((e as CustomEvent<unknown>).detail);
      for (const l of listeners) l();
    });
  }
  tellHost("sa:gmbs-request");
}

export function useHostGmbs(): Gmbs | undefined {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => selected,
  );
}

/** Under the elements' gate, which waits for the host's first `sa:gmbs`. */
export function useGmbs(): Gmbs {
  const gmbs = useHostGmbs();
  if (gmbs === undefined) throw new Error("rendered before the host told its GMBs: outside the gate");
  return gmbs;
}

/** The selection as the API's `gmb` takes it (`Gmbs`): comma-joined, `""` selecting none, absent narrowing nothing. */
export const gmbParam = (gmbs: Gmbs): string | null => (gmbs === null ? null : gmbs.join(","));
