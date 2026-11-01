import { Button, Calendar, Popover, PopoverContent, PopoverTrigger } from "@evinvest/uikit";
import { CalendarDays } from "lucide-react";
import { useState } from "react";

import { useLocale } from "@/shared/i18n";
import { formatDay } from "@/shared/lib/format";
import { dateOfDay, dayOfDate } from "@/shared/lib/instant";

import { useButtonSize } from "./touch";

export interface DayPickerProps {
  id: string;
  /** `YYYY-MM-DD`, or "" for none yet. */
  value: string;
  onChange: (day: string) => void;
  placeholder: string;
  labels: { previous: string; next: string };
  /** Earliest day offered, the same unit. */
  min?: string;
  invalid?: boolean;
  describedBy?: string;
}

/** A calendar day from the kit's Calendar, in a popover: no browser date control. */
export function DayPicker({ id, value, onChange, placeholder, labels, min, invalid, describedBy }: DayPickerProps) {
  const locale = useLocale();
  const button = useButtonSize();
  const [open, setOpen] = useState(false);
  const selected = dateOfDay(value);
  const earliest = min === undefined ? undefined : dateOfDay(min);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          type="button"
          variant="outline"
          size={button()}
          className="w-full justify-start"
          aria-invalid={invalid ? true : undefined}
          {...(describedBy === undefined ? {} : { "aria-describedby": describedBy })}
        >
          <CalendarDays aria-hidden className="size-4" />
          {selected ? formatDay(value, locale) : placeholder}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-auto p-2">
        <Calendar
          {...(selected ? { selected, defaultMonth: selected } : {})}
          {...(earliest ? { min: earliest } : {})}
          locale={locale}
          previousMonthLabel={labels.previous}
          nextMonthLabel={labels.next}
          onSelect={(date) => {
            onChange(dayOfDate(date));
            setOpen(false);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}
