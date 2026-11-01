import { Alert, AlertDescription, AlertTitle, Button } from "@evinvest/uikit";

import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { useButtonSize } from "@/shared/ui/touch";

import type { PlaceSettingsView } from "../model/settings";

/**
 * Someone saved the place while this form holds edits. The edits stay; saving
 * them now would be refused (409), so the way on is to load the fresh data.
 */
export function StaleAlert({ fresher, onLoad }: { fresher: PlaceSettingsView; onLoad: () => void }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  const at = fresher.updated_at === null ? "—" : formatDateTime(fresher.updated_at, locale);
  return (
    <Alert className="flex flex-col gap-2 border-accent-warn" data-enter="rise">
      <AlertTitle>{t("placeSettings.stale.title")}</AlertTitle>
      <AlertDescription>{t("placeSettings.stale.body", { at, by: fresher.updated_by ?? "—" })}</AlertDescription>
      <Button type="button" variant="outline" size={button("sm")} className="self-start" onClick={onLoad}>
        {t("placeSettings.stale.load")}
      </Button>
    </Alert>
  );
}
