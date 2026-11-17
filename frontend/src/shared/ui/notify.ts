import { toast } from "@evinvest/uikit";

import type { Failure } from "@/shared/api";
import type { T } from "@/shared/i18n";

import { failureText } from "./failure-text";

/** A failed write, told as a toast; a 401 has already sent the browser to sign in. */
export function notifyFailure(failure: Failure, t: T): void {
  if (failure.code !== "unauthenticated") toast.error(failureText(failure, t));
}
