import { Field, FieldLabel, Select, SelectContent, SelectItem, SelectTrigger, SelectValue, Textarea } from "@evinvest/uikit";
import { useId, useState } from "react";

import type { StageMove } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import { LOST_REASONS, type LostReason } from "../model/moves";
import { FormButtons } from "./quote-form";

export function LostForm({ busy, onSubmit, onCancel }: { busy: boolean; onSubmit: (m: StageMove) => void; onCancel: () => void }) {
  const t = useT();
  const id = useId();
  const [reason, setReason] = useState<LostReason | "">("");
  const [note, setNote] = useState("");
  const size = useControlSize();

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (!reason) return;
        onSubmit(note.trim() ? { stage: "lost", reason, note: note.trim() } : { stage: "lost", reason });
      }}
    >
      <Field className="flex flex-col gap-1">
        <FieldLabel htmlFor={`${id}-reason`}>{t("move.reason")}</FieldLabel>
        <Select value={reason} onValueChange={(v) => setReason(LOST_REASONS.find((r) => r === v) ?? "")}>
          <SelectTrigger id={`${id}-reason`} size={size} className="w-full">
            <SelectValue placeholder={t("move.reason")} />
          </SelectTrigger>
          <SelectContent>
            {LOST_REASONS.map((r) => (
              <SelectItem key={r} value={r}>
                {t(`lost.${r}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
      <Field className="flex flex-col gap-1">
        <FieldLabel htmlFor={`${id}-note`}>{t("move.note")}</FieldLabel>
        <Textarea id={`${id}-note`} size={size} rows={2} maxLength={1000} value={note} onChange={(e) => setNote(e.target.value)} />
      </Field>
      <FormButtons busy={busy || !reason} onCancel={onCancel} />
    </form>
  );
}
