import { Field, FieldLabel, Select, SelectContent, SelectItem, SelectTrigger, SelectValue, cn } from "@evinvest/uikit";
import { useId } from "react";

import { CURRENCIES } from "@/shared/config/money";

import { useControlSize } from "./touch";

/** The currency beside an amount: one of the currencies the panel bills in. */
export function CurrencyField({ label, value, onChange, className }: { label: string; value: string; onChange: (currency: string) => void; className?: string }) {
  const id = useId();
  const size = useControlSize();
  return (
    <Field className={cn("flex flex-col gap-1", className)}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger id={id} size={size} className="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {CURRENCIES.map((c) => (
            <SelectItem key={c} value={c}>
              {c}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </Field>
  );
}
