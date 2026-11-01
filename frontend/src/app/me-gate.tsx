import type { ReactNode } from "react";

import { MeProvider, fetchMe } from "@/entities/session";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";

/** Renders `children` once `/me` answered; `waiting` meanwhile and after a 401 (the host is signing the person in). */
export function MeGate({ waiting, children }: { waiting: ReactNode; children: ReactNode }) {
  const me = useResource("me", fetchMe);
  if (me.status === "loading" || (me.status === "error" && me.failure.kind === "unauthenticated")) return waiting;
  if (me.status === "error") return <ErrorState failure={me.failure} onRetry={me.reload} />;
  return <MeProvider value={me.data}>{children}</MeProvider>;
}
