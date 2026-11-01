import { Field, FieldDescription, FieldLabel, Input, cn } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import type { FieldErrors } from "../model/errors";
import { describedByOf, domIdOf, hintIdOf } from "../model/fields";

import { FieldMessages } from "./field-messages";

export interface TextFieldProps {
  /** The editor's name for the field: its errors and its DOM id. */
  field: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  errors: FieldErrors;
  hint?: string;
  /**
   * The row the field belongs to ("Studio"), when the form repeats the label:
   * a screen reader then hears "Studio — Adds, €", not five "Adds, €".
   */
  context?: string | null;
  /** Decimal amounts get the phone's number pad; slugs are typed in a fixed font. */
  kind?: "text" | "amount" | "slug";
  className?: string;
}

/** One labelled input of the editor, its reasons under it. */
export function TextField({ field, label, value, onChange, errors, hint, context, kind = "text", className }: TextFieldProps) {
  const t = useT();
  const size = useControlSize();
  const shown = errors.byField.get(field);
  const id = domIdOf(field);
  const describedBy = describedByOf(field, hint !== undefined, shown?.length ?? 0);
  return (
    <Field className={cn("flex min-w-0 flex-col gap-1", className)} data-invalid={shown ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input
        id={id}
        size={size}
        value={value}
        autoComplete="off"
        spellCheck={kind === "text"}
        inputMode={kind === "amount" ? "decimal" : "text"}
        className={cn("w-full", kind === "slug" && "font-mono")}
        aria-invalid={shown ? true : undefined}
        {...(context ? { "aria-label": t("pricing.field.named", { name: context, field: label }) } : {})}
        {...(describedBy === undefined ? {} : { "aria-describedby": describedBy })}
        onChange={(e) => onChange(kind === "slug" ? e.target.value.toLowerCase() : e.target.value)}
      />
      {hint !== undefined && <FieldDescription id={hintIdOf(field)}>{hint}</FieldDescription>}
      <FieldMessages shown={shown} field={field} />
    </Field>
  );
}
