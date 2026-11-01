import { Field, FieldDescription, FieldError, FieldLabel, FieldLegend, FieldSet, Input, Select, SelectContent, SelectItem, SelectTrigger } from "@evinvest/uikit";
import { useId } from "react";

import { BOOKING_PROVIDERS, type BookingProvider, PAGE_PROVIDERS, type PageProvider } from "@/shared/config/booking";
import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

import { type BookingDraft, bookingDraftProblems, urlKey } from "../model/booking-draft";

const SITE_OWN = "__site__";

const PLACEHOLDER: Record<PageProvider, string> = {
  link: "https://book.example.fr/…",
  google_calendar: "https://calendar.app.google/…",
  cal_com: "https://cal.com/<user>/<event>",
};

export interface BookingFieldsProps {
  draft: BookingDraft;
  onChange: (draft: BookingDraft) => void;
  /** The server's 422 reasons by key (`booking.default`, `booking.providers.link.url`, …). */
  serverErrors: Readonly<Record<string, string>>;
}

/**
 * The booking a place offers: its default, and a page per provider that has
 * one. Checked as typed by the server's own rules, so a refused link is said
 * on its field before the save; the server's reason wins when it gives one.
 */
export function BookingFields({ draft, onChange, serverErrors }: BookingFieldsProps) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  const problems = bookingDraftProblems(draft);
  const reason = (key: string) => serverErrors[key] ?? (problems.has(key) ? t(`placeSettings.booking.problem.${problems.get(key) ?? "shape"}`) : null);
  const shown = new Set(["booking.default", ...PAGE_PROVIDERS.map(urlKey)]);
  // Keys the fields below do not show (the whole setting, a provider entry): said under the legend.
  const rest = [...new Set([...Object.keys(serverErrors), ...problems.keys()])].filter((k) => (k === "booking" || k.startsWith("booking.")) && !shown.has(k));
  const pick = (v: string) => onChange({ ...draft, default: BOOKING_PROVIDERS.find((p) => p === v) ?? null });
  const defaultReason = reason("booking.default");

  return (
    <FieldSet className="flex flex-col gap-3">
      <FieldLegend>{t("placeSettings.booking.legend")}</FieldLegend>
      {rest.map((k) => (
        <FieldError key={k}>{reason(k)}</FieldError>
      ))}
      <Field className="flex flex-col gap-1" data-invalid={defaultReason ? true : undefined}>
        <FieldLabel htmlFor={`${id}-default`}>{t("placeSettings.booking.default")}</FieldLabel>
        <Select value={draft.default ?? SITE_OWN} onValueChange={pick}>
          <SelectTrigger id={`${id}-default`} size={size} className="w-full" aria-invalid={defaultReason ? true : undefined}>
            {/* The kit's SelectValue shows the stored word; the label is what a person reads. */}
            <span>{draft.default ? t(`booking.provider.${draft.default}`) : t("placeSettings.booking.siteOwn")}</span>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={SITE_OWN}>{t("placeSettings.booking.siteOwn")}</SelectItem>
            {BOOKING_PROVIDERS.map((p: BookingProvider) => (
              <SelectItem key={p} value={p}>
                {t(`booking.provider.${p}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <FieldDescription>{t("placeSettings.booking.manualHint")}</FieldDescription>
        {defaultReason && <FieldError>{defaultReason}</FieldError>}
      </Field>
      <FieldDescription>{t("placeSettings.booking.pagesHint")}</FieldDescription>
      {PAGE_PROVIDERS.map((p) => {
        const why = reason(urlKey(p));
        return (
          <Field key={p} className="flex flex-col gap-1" data-invalid={why ? true : undefined}>
            <FieldLabel htmlFor={`${id}-${p}`}>{t(`booking.provider.${p}`)}</FieldLabel>
            <Input
              id={`${id}-${p}`}
              type="url"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              size={size}
              value={draft.urls[p]}
              placeholder={PLACEHOLDER[p]}
              aria-invalid={why ? true : undefined}
              onChange={(e) => onChange({ ...draft, urls: { ...draft.urls, [p]: e.target.value } })}
            />
            {why && <FieldError>{why}</FieldError>}
          </Field>
        );
      })}
    </FieldSet>
  );
}
