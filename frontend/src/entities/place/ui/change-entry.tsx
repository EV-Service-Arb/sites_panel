import type { ReactNode } from "react";

import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";

import { diffLineText, diffSettings } from "../lib/diff";
import type { SettingsChange } from "../model/settings";

/** One change of a place's live data: who and when, then each field before → after. */
export function ChangeEntry({ change, action }: { change: SettingsChange; action?: ReactNode }) {
  const t = useT();
  const locale = useLocale();
  const lines = diffSettings(change.before, change.after, t);
  return (
    <li className="flex flex-col gap-2 border-b border-border py-3 last:border-b-0">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-sm text-ink-mid">{t("placeSettings.history.by", { at: formatDateTime(change.at, locale), by: change.by })}</span>
        {action}
      </div>
      {lines.length === 0 ? (
        <p className="text-sm text-ink-soft">{t("placeSettings.history.noDiff")}</p>
      ) : (
        <ul className="flex flex-col gap-1 text-sm text-ink">
          {lines.map((line) => (
            <li key={line.field} className="wrap-anywhere">
              {diffLineText(line, t)}
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}
