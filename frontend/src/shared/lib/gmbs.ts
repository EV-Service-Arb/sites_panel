import { useSyncExternalStore } from "react";

import { GMBS_EVENT, tellHost } from "@/shared/lib/host-events";
import { arrayOf, nullable, num, object, parse } from "@/shared/lib/parse";

/** The GMBs the host's outline selects: `null` narrows nothing; `undefined` until the host has said. */
export type Gmbs = readonly number[] | null;

let selected: Gmbs | undefined;
const listeners = new Set<() => void>();
let installed = false;

const detailParser = object({ ids: nullable(arrayOf(num)) });

/** Called by each element as it connects: the window listener once per page, the selection asked for each time. */
export function followHostGmbs(): void {
  if (!installed) {
    installed = true;
    window.addEventListener(GMBS_EVENT, (e) => {
      selected = parse(detailParser, (e as CustomEvent<unknown>).detail).ids;
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

/** A key part naming the selection, for reads that follow it. */
export const gmbsKey = (gmbs: Gmbs): string => (gmbs === null ? "*" : gmbs.join(","));
