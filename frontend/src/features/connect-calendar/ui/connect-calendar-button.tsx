import { Button } from "@evinvest/uikit";
import { CalendarCheck } from "lucide-react";
import { useState } from "react";

import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * The brand's calendar owner gives the panel read access to it at Google: the server answers Google's consent page,
 * which comes back to this page once they said yes or no.
 */
export function ConnectCalendarButton({ brand }: { brand: string }) {
  const t = useT();
  const button = useButtonSize();
  const [busy, setBusy] = useState(false);
  const connect = async () => {
    setBusy(true);
    const { data, error } = await api().POST("/bookings/google_calendar/{brand}/consent", { params: { path: { brand } }, body: { return_to: window.location.href } });
    setBusy(false);
    if (error !== undefined) return notifyFailure(error, t);
    window.location.assign(data.url);
  };
  return (
    <div className="flex flex-col items-start gap-1">
      <Button type="button" variant="outline" size={button("sm")} disabled={busy} onClick={() => void connect()}>
        <CalendarCheck aria-hidden />
        {t("calendar.connect")}
      </Button>
      <p className="text-sm text-ink-soft">{t("calendar.connect.hint", { brand })}</p>
    </div>
  );
}
