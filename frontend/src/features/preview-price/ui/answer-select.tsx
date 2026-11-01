import { Field, FieldLabel, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";
import { useId } from "react";

import { useControlSize } from "@/shared/ui/touch";

/** One choice of the preview — the need, or an answer to one question — as the visitor would make it. */
export function AnswerSelect({ label, value, options, onChange }: { label: string; value: string; options: readonly { value: string; label: string }[]; onChange: (v: string) => void }) {
  const id = useId();
  const size = useControlSize();
  return (
    <Field className="flex min-w-0 flex-col gap-1">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger id={id} size={size} className="w-full">
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
    </Field>
  );
}
