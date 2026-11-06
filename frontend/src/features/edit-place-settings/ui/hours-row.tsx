import { DAYS } from "@evinvest/sites-contract";
import { Button, FieldError, ToggleGroup, ToggleGroupItem } from "@evinvest/uikit";
import { X } from "lucide-react";
import { useId } from "react";

import { formatDays } from "@/entities/place";
import { type T, useT } from "@/shared/i18n";
import { TimeInput } from "@/shared/ui/time-input";
import { useButtonSize, useControlSize } from "@/shared/ui/touch";

import { type HoursDraftRow, type RowProblem, rowProblems } from "../model/hours-draft";

const problemText = (p: RowProblem, t: T): string => ("days" in p ? t("placeSettings.hours.overlap", { days: formatDays(p.days, t) }) : t(`placeSettings.problem.${p.code}`));

export interface HoursRowProps {
  rows: readonly HoursDraftRow[];
  row: HoursDraftRow;
  onChange: (patch: Partial<Omit<HoursDraftRow, "key">>) => void;
  onRemove: () => void;
}

/** One row: the days it covers, then when it opens and closes. */
export function HoursRowEditor({ rows, row, onChange, onRemove }: HoursRowProps) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  const button = useButtonSize();
  const problems = rowProblems(rows, row);
  // Two "bad time" lines would say the same thing twice.
  const reasons = [...new Set(problems.map((p) => problemText(p, t)))];
  return (
    <fieldset className="flex flex-col gap-2 rounded-md border border-border p-3">
      <legend className="sr-only">{t("placeSettings.field.hours")}</legend>
      <ToggleGroup
        type="multiple"
        variant="outline"
        size={size}
        aria-label={t("placeSettings.hours.days")}
        className="flex-wrap"
        value={row.days}
        onValueChange={(v) => onChange({ days: DAYS.filter((d) => (Array.isArray(v) ? v : [v]).includes(d)) })}
      >
        {DAYS.map((d) => (
          <ToggleGroupItem key={d} value={d} aria-label={d}>
            {t(`day.short.${d}`)}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <div className="flex items-end gap-2">
        <TimeInput id={`${id}-opens`} label={t("placeSettings.hours.opens")} value={row.opens} onChange={(opens) => onChange({ opens })} />
        <TimeInput id={`${id}-closes`} label={t("placeSettings.hours.closes")} value={row.closes} onChange={(closes) => onChange({ closes })} />
        <Button type="button" variant="ghost" size={button("md")} className="ml-auto" aria-label={t("placeSettings.hours.remove")} onClick={onRemove}>
          <X aria-hidden="true" />
        </Button>
      </div>
      {reasons.map((r) => (
        <FieldError key={r}>{r}</FieldError>
      ))}
    </fieldset>
  );
}
