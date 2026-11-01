import { Alert, AlertDescription, AlertTitle, Button } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/** A write refused with 409: someone saved the place since it was read. Reloading drops the local state. */
export function ConflictAlert({ onReload }: { onReload: () => void }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <Alert variant="destructive" className="flex flex-col gap-2">
      <AlertTitle>{t("placeSettings.conflict.title")}</AlertTitle>
      <AlertDescription>{t("placeSettings.conflict.body")}</AlertDescription>
      <Button type="button" variant="outline" size={button("sm")} className="self-start" onClick={onReload}>
        {t("placeSettings.conflict.reload")}
      </Button>
    </Alert>
  );
}
