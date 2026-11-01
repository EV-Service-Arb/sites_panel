import { Field, FieldLabel, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import type { FieldErrors } from "../model/errors";
import { describedByOf, domIdOf } from "../model/fields";
import { FieldMessages } from "./field-messages";

export interface KindSelectProps<K extends string> {
  field: string;
  label: string;
  value: K;
  options: readonly { value: K; label: string }[];
  onChange: (value: K) => void;
  errors: FieldErrors;
  /** The row the select belongs to, when the form repeats its label (see `TextField`). */
  context?: string | null;
}

/** A closed choice of the model (an input's kind, a need's): the kit's Select, its reasons under it. */
export function KindSelect<K extends string>({ field, label, value, options, onChange, errors, context }: KindSelectProps<K>) {
  const t = useT();
  const size = useControlSize();
  const shown = errors.byField.get(field);
  const id = domIdOf(field);
  const describedBy = describedByOf(field, false, shown?.length ?? 0);
  return (
    <Field className="flex min-w-0 flex-col gap-1" data-invalid={shown ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select value={value} onValueChange={(v) => onChange(options.find((o) => o.value === v)?.value ?? value)}>
        <SelectTrigger id={id} size={size} className="w-full" aria-invalid={shown ? true : undefined}
          {...(context ? { "aria-label": t("pricing.field.named", { name: context, field: label }) } : {})}
          {...(describedBy === undefined ? {} : { "aria-describedby": describedBy })}
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((o) => (
            <SelectItem key={o.value} value={o.value}>
              {o.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <FieldMessages shown={shown} field={field} />
    </Field>
  );
}
