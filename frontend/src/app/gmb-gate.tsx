import type { ReactNode } from "react";

import { useHostGmbs } from "@/shared/lib/gmbs";

/** Renders `children` once the host told which GMBs its outline selects; every read narrows to them. */
export function GmbGate({ waiting, children }: { waiting: ReactNode; children: ReactNode }) {
  return useHostGmbs() === undefined ? waiting : children;
}
