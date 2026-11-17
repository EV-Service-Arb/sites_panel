import { PRICING_CURRENCY } from "@evinvest/sites-contract";
import { Alert, AlertDescription, Spinner } from "@evinvest/uikit";

import { useLocale, useT } from "@/shared/i18n";
import { formatCents } from "@/shared/lib/format";
import { failureText } from "@/shared/ui/failure-text";

import type { PreviewState } from "../model/use-preview";

/** The server's answer for the combination: a price, "no price", or what in the model stops it. */
export function PreviewResult({ state }: { state: PreviewState }) {
  const t = useT();
  const locale = useLocale();
  switch (state.kind) {
    case "idle":
      return null;
    case "pending":
      return (
        <p className="flex items-center gap-2 text-sm text-ink-soft" aria-live="polite">
          <Spinner className="size-4" /> {t("pricing.preview.pending")}
        </p>
      );
    case "priced":
      return (
        <p className="flex flex-col gap-0.5" aria-live="polite">
          <span className="text-2xl font-semibold text-ink tabular-nums">{state.cents === null ? t("pricing.preview.none") : formatCents(state.cents, PRICING_CURRENCY, locale)}</span>
          {state.cents === null && <span className="text-sm text-ink-soft">{t("pricing.preview.noneBody")}</span>}
        </p>
      );
    case "invalid":
      return (
        <Alert tone="error">
          <AlertDescription className="wrap-anywhere">{t("pricing.preview.invalid", { message: state.message })}</AlertDescription>
        </Alert>
      );
    case "failed":
      return (
        <Alert tone="error">
          <AlertDescription>{failureText(state.failure, t)}</AlertDescription>
        </Alert>
      );
  }
}
