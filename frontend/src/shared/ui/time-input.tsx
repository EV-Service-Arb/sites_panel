import { Field, FieldLabel, Input } from "@evinvest/uikit";

import { normaliseTime } from "@/shared/lib/clock";

import { useControlSize } from "./touch";

/** Text, not `type="time"`: the browser's own picker ignores the kit's tokens and differs per OS. */
export function TimeInput({ id, label, value, invalid, onChange }: { id: string; label: string; value: string; invalid?: boolean; onChange: (v: string) => void }) {
  const size = useControlSize();
  return (
    // The kit's vertical Field stretches its children (`[&>*]:w-full`): the width goes on the Field.
    <Field className="flex w-24 flex-none flex-col gap-1">
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input
        id={id}
        size={size}
        className="tabular-nums"
        inputMode="numeric"
        maxLength={5}
        placeholder="08:00"
        value={value}
        aria-invalid={invalid ? true : undefined}
        onChange={(e) => onChange(e.target.value)}
        onBlur={(e) => onChange(normaliseTime(e.target.value))}
      />
    </Field>
  );
}
