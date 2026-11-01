import { Field, FieldDescription, FieldError, FieldLabel, FieldLegend, FieldSet, Input } from "@evinvest/uikit";
import { useId } from "react";

import { sharesOf } from "@/entities/experiment";
import { useLocale, useT } from "@/shared/i18n";
import { formatPercent } from "@/shared/lib/format";
import { useControlSize } from "@/shared/ui/touch";

import { parseDecimal } from "../model/draft";

/** A weight per variant, the share it comes to beside it as it is typed. The variants are the code's: none is added here. */
export function WeightsFields({ variants, weights, invalid, onChange }: { variants: readonly string[]; weights: readonly string[]; invalid: boolean; onChange: (weights: string[]) => void }) {
  const t = useT();
  const locale = useLocale();
  const id = useId();
  const size = useControlSize();
  const parsed = weights.map((w) => parseDecimal(w) ?? 0);
  const shares = sharesOf(parsed);
  return (
    <FieldSet className="flex flex-col gap-2" data-invalid={invalid ? true : undefined}>
      <FieldLegend variant="label">{t("experiments.edit.weights")}</FieldLegend>
      <FieldDescription>{t("experiments.edit.weights.hint")}</FieldDescription>
      {variants.map((variant, i) => (
        <Field key={variant} orientation="horizontal" className="items-center gap-3">
          <FieldLabel htmlFor={`${id}-${i}`} className="min-w-0 flex-1 break-all font-mono text-sm">
            {variant}
            {i === 0 && <span className="ml-2 font-sans text-xs text-ink-soft">{t("experiments.control")}</span>}
          </FieldLabel>
          <Input
            id={`${id}-${i}`}
            size={size}
            inputMode="decimal"
            autoComplete="off"
            className="w-24 text-right tabular-nums"
            value={weights[i] ?? ""}
            aria-invalid={invalid ? true : undefined}
            onChange={(e) => onChange(weights.map((w, j) => (j === i ? e.target.value : w)))}
          />
          <span className="w-16 text-right text-sm tabular-nums text-ink-mid">{formatPercent(shares[i] ?? 0, locale)}</span>
        </Field>
      ))}
      {invalid && <FieldError>{t("experiments.edit.weights.invalid")}</FieldError>}
    </FieldSet>
  );
}
