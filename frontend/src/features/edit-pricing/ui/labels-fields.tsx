import { useT } from "@/shared/i18n";

import type { FieldErrors } from "../model/errors";
import { domIdOf } from "../model/fields";
import { FieldMessages } from "./field-messages";
import { TextField } from "./text-field";

export interface LabelsFieldsProps {
  labels: Record<string, string>;
  /** The brand's site locales; a label in another (a pasted model's) is kept and shown too. */
  locales: readonly string[];
  /** The group's field (`input:k3:labels`); each locale's is inside it. */
  field: string;
  onChange: (labels: Record<string, string>) => void;
  errors: FieldErrors;
  /** The row the labels belong to (see `TextField`): "Studio — Label (FR)". */
  context?: string | null;
}

/** The words of an input or an option, one per locale: every site locale must have them. */
export function LabelsFields({ labels, locales, field, onChange, errors, context }: LabelsFieldsProps) {
  const t = useT();
  const all = [...locales, ...Object.keys(labels).filter((l) => !locales.includes(l))];
  return (
    <div id={domIdOf(field)} tabIndex={-1} className="flex flex-col gap-1 outline-none">
      <div className="grid gap-2 sm:grid-cols-2">
        {all.map((locale) => (
          <TextField
            key={locale}
            field={`${field}:${locale}`}
            label={t(locales.includes(locale) ? "pricing.field.label" : "pricing.field.labelExtra", { locale: locale.toUpperCase() })}
            value={labels[locale] ?? ""}
            onChange={(text) => onChange({ ...labels, [locale]: text })}
            errors={errors}
            context={context ?? null}
          />
        ))}
      </div>
      <FieldMessages shown={errors.byField.get(field)} />
    </div>
  );
}
