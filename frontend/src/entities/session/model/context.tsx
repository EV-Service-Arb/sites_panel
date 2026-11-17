import { createContext, useContext } from "react";

import type { Me } from "@/shared/api/schema";

const MeContext = createContext<Me | null>(null);

export const MeProvider = MeContext.Provider;

/** The caller; only under an element that renders nothing until it knows them. */
export function useMe(): Me {
  const me = useContext(MeContext);
  if (!me) throw new Error("useMe outside MeProvider");
  return me;
}
