import { Button, Field, FieldDescription, FieldError, FieldLabel } from "@evinvest/uikit";
import { useId, useState } from "react";

import type { LeadBooking, SlotBody } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { dayOfDate } from "@/shared/lib/instant";
import { DayPicker } from "@/shared/ui/day-picker";
import { TimeInput } from "@/shared/ui/time-input";
import { useButtonSize } from "@/shared/ui/touch";

import { type SlotDraft, browserZone, slotBody, slotDraftOf, slotProblems } from "../model/slot-draft";

export interface SlotFormProps {
  booking: LeadBooking;
  busy: boolean;
  onSubmit: (body: SlotBody) => void;
  onCancel: () => void;
}

/**
 * A day from the kit's Calendar and two times, in the browser's zone (named,
 * so nobody books Paris from Moscow by mistake). Problems show once a field
 * was left or the form sent, not while the first digit is typed.
 */
export function SlotForm({ booking, busy, onSubmit, onCancel }: SlotFormProps) {
  const t = useT();
  const id = useId();
  const button = useButtonSize();
  const [draft, setDraft] = useState<SlotDraft>(() => slotDraftOf(booking));
  const [tried, setTried] = useState(false);
  const set = (patch: Partial<SlotDraft>) => setDraft((d) => ({ ...d, ...patch }));
  const problems = tried ? slotProblems(draft) : [];
  const has = (p: (typeof problems)[number]) => problems.includes(p);

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        setTried(true);
        const body = slotBody(draft);
        if (body) onSubmit(body);
      }}
    >
      <FieldDescription>{t("slot.description", { zone: browserZone() })}</FieldDescription>
      <Field className="flex flex-col gap-1" data-invalid={has("day") ? true : undefined}>
        <FieldLabel htmlFor={`${id}-day`}>{t("slot.day")}</FieldLabel>
        <DayPicker
          id={`${id}-day`}
          value={draft.day}
          onChange={(day) => set({ day })}
          placeholder={t("slot.pickDay")}
          labels={{ previous: t("pricing.calendar.previous"), next: t("pricing.calendar.next") }}
          min={dayOfDate(new Date())}
          invalid={has("day")}
        />
      </Field>
      <div className="flex flex-wrap items-end gap-3">
        <TimeInput id={`${id}-start`} label={t("slot.start")} value={draft.start} invalid={has("start") || has("order")} onChange={(start) => set({ start })} />
        <TimeInput id={`${id}-end`} label={t("slot.end")} value={draft.end} invalid={has("end") || has("order")} onChange={(end) => set({ end })} />
      </div>
      {problems.map((p) => (
        <FieldError key={p}>{t(`slot.problem.${p}`)}</FieldError>
      ))}
      <div className="flex flex-wrap gap-2">
        <Button type="submit" size={button()} disabled={busy}>
          {t("slot.save")}
        </Button>
        <Button type="button" variant="ghost" size={button()} disabled={busy} onClick={onCancel}>
          {t("slot.cancel")}
        </Button>
      </div>
    </form>
  );
}
