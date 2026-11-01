import { Field, FieldLabel, ToggleGroup, ToggleGroupItem } from "@evinvest/uikit";
import { useId } from "react";

import { ChannelIcon, MANUAL_CHANNELS, type ManualChannel } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

/** How the customer reached us, of the ways an operator takes a lead in by hand: a call, or a message they wrote. */
export function ChannelField({ value, onChange }: { value: ManualChannel; onChange: (v: ManualChannel) => void }) {
  const t = useT();
  const id = useId();
  // The fields around it are `lg` (48px) under the thumb: the segments match.
  const size = useControlSize() === "lg" ? "xl" : "md";
  return (
    <Field className="flex flex-col gap-1">
      <FieldLabel id={id}>{t("create.channel")}</FieldLabel>
      <ToggleGroup
        type="single"
        variant="outline"
        size={size}
        aria-labelledby={id}
        className="w-full"
        value={value}
        // Tapping the lit item would clear the choice; a lead always has a channel.
        onValueChange={(v) => {
          const next = MANUAL_CHANNELS.find((c) => c === v);
          if (next) onChange(next);
        }}
      >
        {MANUAL_CHANNELS.map((c) => (
          <ToggleGroupItem key={c} value={c} className="min-w-0 flex-1 gap-1.5">
            <ChannelIcon channel={c} className="shrink-0" />
            <span className="truncate">{t(`channel.${c}`)}</span>
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </Field>
  );
}
