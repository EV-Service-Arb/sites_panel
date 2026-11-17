import { Field, FieldLabel, Select, SelectContent, SelectItem, SelectTrigger, SelectValue, cn } from "@evinvest/uikit";
import { useId } from "react";

import { type Currency, currencyValues } from "@/shared/api/schema";

import { useControlSize } from "./touch";

/** The currency beside an amount: one of the currencies the panel bills in. */
export function CurrencyField({ label, value, onChange, className }: { label: string; value: Currency; onChange: (currency: Currency) => void; className?: string }) {
  const id = useId();
  const size = useControlSize();
  const pick = (v: string) => {
    const currency = currencyValues.find((c) => c === v);
    if (currency === undefined) throw new Error(`${v} is none of the currencies offered`);
    onChange(currency);
  };
  return (
    <Field className={cn("flex flex-col gap-1", className)}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select value={value} onValueChange={pick}>
        <SelectTrigger id={id} size={size} className="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {currencyValues.map((c) => (
            <SelectItem key={c} value={c}>
              {c}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </Field>
  );
}
