import { EDITED_KEYS } from "@/entities/place";

type FieldKey = (typeof EDITED_KEYS)[number];

export interface FieldErrors {
  /** Per form field: every reason whose key is the field or a path inside it (`hours[0].opens`). */
  byField: Partial<Record<FieldKey, string[]>>;
  /** Every reason by its own key, for a field that shows parts apart (`booking.providers.link.url`). */
  byKey: Readonly<Record<string, string>>;
}

const owner = (key: string): FieldKey | undefined => EDITED_KEYS.find((f) => key === f || key.startsWith(`${f}.`) || key.startsWith(`${f}[`));

/** The form's own reasons by path, sorted onto its fields: it checks only what it edits. */
export function fieldErrorsOf(fields: Record<string, string>): FieldErrors {
  const out: FieldErrors = { byField: {}, byKey: fields };
  for (const [key, reason] of Object.entries(fields)) {
    const f = owner(key);
    if (f === undefined) throw new Error(`${key}: a problem of no field the form edits`);
    out.byField[f] = [...(out.byField[f] ?? []), reason];
  }
  return out;
}
