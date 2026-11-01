import { ResourceError } from "@evinvest/uikit";

import type { ApiFailure } from "@/shared/api";
import { useT } from "@/shared/i18n";

import { failureText } from "./failure-text";

/** A read that never arrived: a line with a retry, or — with nothing to retry — the kit's alert. */
export function ErrorState({ failure, onRetry }: { failure: ApiFailure | { kind: "invalid"; message: string }; onRetry?: () => void }) {
  const t = useT();
  const message = failureText(failure, t);
  if (!onRetry) return <ResourceError variant="alert" message={message} />;
  return <ResourceError message={message} onRetry={onRetry} labels={{ retry: t("state.retry") }} />;
}
