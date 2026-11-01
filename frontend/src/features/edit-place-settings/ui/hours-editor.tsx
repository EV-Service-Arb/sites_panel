import { Button, FieldError } from "@evinvest/uikit";
import { Plus } from "lucide-react";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import { type HoursDraftRow, addHoursRow, removeHoursRow, updateHoursRow } from "../model/hours-draft";
import { HoursRowEditor } from "./hours-row";

/** The opening hours as rows; none means the site keeps its own. */
export function HoursEditor({ rows, onChange, errors }: { rows: HoursDraftRow[]; onChange: (rows: HoursDraftRow[]) => void; errors: string[] | undefined }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <section className="flex flex-col gap-2" aria-label={t("placeSettings.field.hours")}>
      <h3 className="text-sm font-medium text-ink">{t("placeSettings.field.hours")}</h3>
      {rows.length === 0 && <p className="text-sm text-ink-soft">{t("placeSettings.hours.empty")}</p>}
      {rows.map((row) => (
        <HoursRowEditor key={row.key} rows={rows} row={row} onChange={(patch) => onChange(updateHoursRow(rows, row.key, patch))} onRemove={() => onChange(removeHoursRow(rows, row.key))} />
      ))}
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
      <Button type="button" variant="outline" size={button("sm")} className="self-start" onClick={() => onChange(addHoursRow(rows))}>
        <Plus aria-hidden="true" />
        {t("placeSettings.hours.add")}
      </Button>
    </section>
  );
}
