import { Button, Field, FieldLabel, Input } from "@evinvest/uikit";
import { useId, useState } from "react";

import type { StageMove } from "@/entities/lead";
import { DEFAULT_CURRENCY } from "@/shared/config/money";
import { useT } from "@/shared/i18n";
import { CurrencyField } from "@/shared/ui/currency-field";
import { parseMoney } from "@/shared/lib/format";
import { useButtonSize, useControlSize } from "@/shared/ui/touch";

/** A quote with an optional amount: both amount and currency, or neither. */
export function QuoteForm({ busy, onSubmit, onCancel }: { busy: boolean; onSubmit: (m: StageMove) => void; onCancel: () => void }) {
  const t = useT();
  const id = useId();
  const [amount, setAmount] = useState("");
  const [currency, setCurrency] = useState<string>(DEFAULT_CURRENCY);
  const size = useControlSize();
  const minor = amount.trim() === "" ? null : parseMoney(amount);
  const invalid = amount.trim() !== "" && minor === null;

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (invalid) return;
        onSubmit(minor === null ? { stage: "quoted" } : { stage: "quoted", amount: minor, currency });
      }}
    >
      <div className="flex gap-2">
        <Field className="flex flex-1 flex-col gap-1">
          <FieldLabel htmlFor={`${id}-amount`}>{t("move.amount")}</FieldLabel>
          <Input id={`${id}-amount`} size={size} inputMode="decimal" value={amount} aria-invalid={invalid} onChange={(e) => setAmount(e.target.value)} />
        </Field>
        <CurrencyField className="w-28" label={t("move.currency")} value={currency} onChange={setCurrency} />
      </div>
      <FormButtons busy={busy || invalid} onCancel={onCancel} />
    </form>
  );
}

export function FormButtons({ busy, onCancel, submitLabel }: { busy: boolean; onCancel: () => void; submitLabel?: string }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <div className="flex gap-2">
      <Button type="submit" size={button()} disabled={busy}>
        {submitLabel ?? t("move.submit")}
      </Button>
      <Button type="button" variant="ghost" size={button()} onClick={onCancel}>
        {t("move.cancel")}
      </Button>
    </div>
  );
}
