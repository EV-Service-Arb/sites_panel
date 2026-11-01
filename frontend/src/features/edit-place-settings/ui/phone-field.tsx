import { Field, FieldDescription, FieldError, FieldLabel, Input } from "@evinvest/uikit";
import { useId } from "react";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import { looksLikeE164 } from "../model/draft";

/** A number in E.164: a soft hint while typing, the server's own reason once it refuses. */
export function PhoneField({ label, value, onChange, errors }: { label: string; value: string; onChange: (v: string) => void; errors: string[] | undefined }) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  const doubtful = !looksLikeE164(value);
  return (
    <Field className="flex flex-col gap-1" data-invalid={errors ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input id={id} type="tel" inputMode="tel" autoComplete="off" size={size} value={value} placeholder="+33612345678" aria-invalid={errors ? true : undefined} onChange={(e) => onChange(e.target.value)} />
      {doubtful ? <FieldDescription className="text-accent-warn">{t("placeSettings.phone.notE164")}</FieldDescription> : <FieldDescription>{t("placeSettings.phone.hint")}</FieldDescription>}
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
    </Field>
  );
}
