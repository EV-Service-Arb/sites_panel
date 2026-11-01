import { Badge } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useNow } from "@/shared/lib/use-now";

import { PLACE_TIME_ZONE } from "../config/time-zone";
import { type ChannelSlot, channelPreview } from "../lib/channels";
import { localTimeOf } from "../lib/hours";
import type { EditedFields } from "../model/settings";

const clock = (minutes: number) => `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;

/**
 * The contact channels the site will lead with, in order, for the fields given —
 * the saved ones or the form's unsaved ones — at the place's time now.
 */
export function ChannelPreview({ fields }: { fields: EditedFields }) {
  const t = useT();
  const now = useNow();
  const at = localTimeOf(new Date(now), PLACE_TIME_ZONE);
  const preview = channelPreview(fields, at);
  return (
    <section className="flex flex-col gap-2 rounded-md border border-border p-3" aria-label={t("placeSettings.preview.title")}>
      <h3 className="text-xs font-medium uppercase tracking-wide text-ink-soft">{t("placeSettings.preview.title")}</h3>
      <p className="text-sm text-ink-mid">{t(`placeSettings.preview.${preview.state}`, { time: clock(at.minutes) })}</p>
      <ol className="flex flex-col gap-1">
        {preview.slots.map((slot, i) => (
          <SlotLine key={slot.channel} slot={slot} n={i + 1} />
        ))}
      </ol>
    </section>
  );
}

function SlotLine({ slot, n }: { slot: ChannelSlot; n: number }) {
  const t = useT();
  const detail = slot.channel === "callback" ? t("placeSettings.preview.always") : (slot.value ?? t("placeSettings.preview.siteOwn"));
  return (
    <li className="flex flex-wrap items-center gap-2 text-sm">
      <Badge variant={n === 1 ? "primary" : "outline"} className="tabular-nums">
        {n}
      </Badge>
      <span className="text-ink">{t(`placeSettings.preview.${slot.channel}`)}</span>
      <span className={slot.value ? "font-mono text-ink-mid" : "text-ink-soft italic"}>{detail}</span>
    </li>
  );
}
