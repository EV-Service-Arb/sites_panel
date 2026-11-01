import { Field, FieldLabel } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { DayPicker } from "@/shared/ui/day-picker";

import type { FieldErrors } from "../model/errors";
import { describedByOf, domIdOf } from "../model/fields";
import { FieldMessages } from "./field-messages";

/** A model's day field: the shared day picker, with the editor's messages under it. */
export function DayField({ field, label, value, onChange, errors }: { field: string; label: string; value: string; onChange: (day: string) => void; errors: FieldErrors }) {
  const t = useT();
  const shown = errors.byField.get(field);
  const id = domIdOf(field);
  const describedBy = describedByOf(field, false, shown?.length ?? 0);
  return (
    <Field className="flex min-w-0 flex-col gap-1" data-invalid={shown ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <DayPicker
        id={id}
        value={value}
        onChange={onChange}
        placeholder={t("pricing.field.pickDay")}
        labels={{ previous: t("pricing.calendar.previous"), next: t("pricing.calendar.next") }}
        invalid={shown !== undefined}
        {...(describedBy === undefined ? {} : { describedBy })}
      />
      <FieldMessages shown={shown} field={field} />
    </Field>
  );
}
