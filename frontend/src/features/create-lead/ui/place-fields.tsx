import { Field, FieldLabel, Input, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";
import { useId } from "react";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

const OTHER = "__other__";

export interface PlaceValue {
  brand: string;
  /** A known location, or `null` while "Other…" is chosen and `custom` holds the id. */
  location: string | null;
  custom: string;
}

/** Brand, then location among those the brand is known to have, with "Other…" for a new one. */
export function PlaceFields({ brands, locations, value, onChange }: { brands: string[]; locations: string[]; value: PlaceValue; onChange: (v: PlaceValue) => void }) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  return (
    <div className="grid grid-cols-2 gap-2">
      <Field className="flex flex-col gap-1">
        <FieldLabel htmlFor={`${id}-brand`}>{t("create.brand")}</FieldLabel>
        <Select value={value.brand} onValueChange={(brand) => onChange({ brand, location: null, custom: "" })}>
          <SelectTrigger id={`${id}-brand`} size={size} className="w-full">
            <SelectValue placeholder={t("create.brand")} />
          </SelectTrigger>
          <SelectContent>
            {brands.map((b) => (
              <SelectItem key={b} value={b}>
                {b}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
      <Field className="flex flex-col gap-1">
        <FieldLabel htmlFor={`${id}-location`}>{t("create.location")}</FieldLabel>
        <Select value={value.location ?? (value.brand ? OTHER : "")} onValueChange={(v) => onChange({ ...value, location: v === OTHER ? null : v })}>
          <SelectTrigger id={`${id}-location`} size={size} className="w-full" disabled={!value.brand}>
            <SelectValue placeholder={t("create.location")} />
          </SelectTrigger>
          <SelectContent>
            {locations.map((l) => (
              <SelectItem key={l} value={l}>
                {l}
              </SelectItem>
            ))}
            <SelectItem value={OTHER}>{t("create.location.other")}</SelectItem>
          </SelectContent>
        </Select>
      </Field>
      {value.brand && value.location === null && (
        <Field className="col-span-2 flex flex-col gap-1">
          <FieldLabel htmlFor={`${id}-slug`}>{t("create.location.slug")}</FieldLabel>
          <Input id={`${id}-slug`} size={size} autoCapitalize="none" value={value.custom} onChange={(e) => onChange({ ...value, custom: e.target.value.toLowerCase() })} />
        </Field>
      )}
    </div>
  );
}
