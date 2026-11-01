import { createContext, useContext } from "react";

import type { Me, Permission } from "@/shared/api";

const MeContext = createContext<Me | null>(null);

export const MeProvider = MeContext.Provider;

/** The caller; only under an element that renders nothing until it knows them. */
export function useMe(): Me {
  const me = useContext(MeContext);
  if (!me) throw new Error("useMe outside MeProvider");
  return me;
}

/** Whether the caller holds `permission`: the set the conductor asserts is concrete. */
export function may(caller: Pick<Me, "permissions">, permission: Permission): boolean {
  return caller.permissions.includes(permission);
}
