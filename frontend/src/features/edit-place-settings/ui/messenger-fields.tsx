import { Field, FieldDescription, FieldError, FieldLabel, FieldLegend, FieldSet, InputGroup, InputGroupAddon, InputGroupInput, InputGroupText, Label, Switch, cn } from "@evinvest/uikit";
import { useId } from "react";

import { MESSENGER_SWITCHES, type MessengerSwitch } from "@/entities/place";
import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import { looksLikeBot } from "../model/draft";

type Errors = string[] | undefined;

/** The place's Telegram bot, `@` shown before it as people write it but not stored. */
export function TelegramField({ value, onChange, errors }: { value: string; onChange: (v: string) => void; errors: Errors }) {
  const t = useT();
  const id = useId();
  // Same height as the kit's Input beside it: `md` on a desktop, `lg` (48px) under the thumb.
  const tall = useControlSize() === "lg";
  return (
    <Field className="flex flex-col gap-1" data-invalid={errors ? true : undefined}>
      <FieldLabel htmlFor={id}>{t("placeSettings.field.telegram")}</FieldLabel>
      <InputGroup className={cn(tall && "h-12 text-base")}>
        <InputGroupAddon>
          <InputGroupText>@</InputGroupText>
        </InputGroupAddon>
        <InputGroupInput
          id={id}
          autoCapitalize="none"
          autoComplete="off"
          spellCheck={false}
          placeholder="aquafix_devis_bot"
          value={value}
          aria-invalid={errors ? true : undefined}
          // Pasted as "@name" or "t.me/name": keep the name.
          onChange={(e) => onChange(e.target.value.replace(/^\s*(?:https?:\/\/)?(?:t\.me\/)?@?/, ""))}
        />
      </InputGroup>
      {looksLikeBot(value) ? <FieldDescription>{t("placeSettings.telegram.hint")}</FieldDescription> : <FieldDescription className="text-accent-warn">{t("placeSettings.telegram.invalid")}</FieldDescription>}
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
    </Field>
  );
}

/** Kill switches for the lead form's messenger options: off, the form falls back to the phone; other buttons on the site stay. */
export function MessengerSwitches({ value, onChange, errors }: { value: Record<MessengerSwitch, boolean>; onChange: (v: Record<MessengerSwitch, boolean>) => void; errors: Errors }) {
  const t = useT();
  const id = useId();
  return (
    <FieldSet className="flex flex-col gap-2" data-invalid={errors ? true : undefined}>
      <FieldLegend variant="label">{t("placeSettings.field.messengers")}</FieldLegend>
      {MESSENGER_SWITCHES.map((m) => (
        <div key={m} className="flex items-center gap-2 max-md:min-h-11">
          <Switch id={`${id}-${m}`} checked={value[m]} onCheckedChange={(on) => onChange({ ...value, [m]: on })} />
          <Label htmlFor={`${id}-${m}`}>{t(`placeSettings.messengers.${m}`)}</Label>
        </div>
      ))}
      <FieldDescription>{t("placeSettings.messengers.hint")}</FieldDescription>
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
    </FieldSet>
  );
}
