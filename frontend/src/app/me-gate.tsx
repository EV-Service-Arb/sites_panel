import { useResource } from "@evinvest/api/react";
import type { ReactNode } from "react";

import { MeProvider } from "@/entities/session";
import { api } from "@/shared/api";
import { ErrorState } from "@/shared/ui/error-state";

/** Renders `children` once `/me` answered; `waiting` meanwhile and after a 401 (the host is signing the person in). */
export function MeGate({ waiting, children }: { waiting: ReactNode; children: ReactNode }) {
  const me = useResource(api(), "/me");
  if (me.status === "loading" || (me.status === "error" && me.failure.code === "unauthenticated")) return waiting;
  if (me.status === "error") return <ErrorState failure={me.failure} onRetry={me.reload} />;
  return <MeProvider value={me.data}>{children}</MeProvider>;
}
