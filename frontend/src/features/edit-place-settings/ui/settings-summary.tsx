import { ChannelPreview, type PlaceSettingsView, bookingText, formatHours, messengersText } from "@/entities/place";
import { useT } from "@/shared/i18n";

import { KeptFields } from "./kept-fields";

/** The live data as it stands, for someone who may read it and not change it. */
export function SettingsSummary({ place }: { place: PlaceSettingsView }) {
  const t = useT();
  const e = place.settings.edited;
  const own = <span className="italic text-ink-soft">{t("placeSettings.siteOwn")}</span>;
  const rows = [
    { label: t("placeSettings.field.phone"), value: e.phone },
    { label: t("placeSettings.field.whatsapp"), value: e.whatsapp },
    { label: t("placeSettings.field.telegram"), value: e.telegram && `@${e.telegram}` },
    { label: t("placeSettings.field.messengers"), value: messengersText(e, t) ?? t("placeSettings.messengers.allOn") },
    { label: t("placeSettings.field.hours"), value: e.hours && formatHours(e.hours, t) },
    { label: t("placeSettings.field.serviceArea"), value: e.serviceArea?.join(", ") },
    { label: t("placeSettings.field.booking"), value: e.booking && bookingText(e.booking, t) },
  ];
  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-ink-soft">{t("placeSettings.readOnly")}</p>
      <dl className="grid grid-cols-(--grid-place-field) gap-x-3 gap-y-2 text-sm">
        {rows.map((r) => (
          <div key={r.label} className="contents">
            <dt className="text-ink-mid">{r.label}</dt>
            <dd className="wrap-anywhere text-ink">{r.value || own}</dd>
          </div>
        ))}
      </dl>
      <KeptFields rest={place.settings.rest} />
      <ChannelPreview fields={e} />
    </div>
  );
}
