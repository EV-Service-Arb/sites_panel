import { Field, FieldDescription, FieldError, FieldLabel, Input } from "@evinvest/uikit";
import { useId } from "react";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

/** A number in E.164. */
export function PhoneField({ label, value, onChange, errors }: { label: string; value: string; onChange: (v: string) => void; errors: string[] | undefined }) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  return (
    <Field className="flex flex-col gap-1" data-invalid={errors ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input id={id} type="tel" inputMode="tel" autoComplete="off" size={size} value={value} placeholder="+33612345678" aria-invalid={errors ? true : undefined} onChange={(e) => onChange(e.target.value)} />
      <FieldDescription>{t("placeSettings.phone.hint")}</FieldDescription>
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
    </Field>
  );
}
