import { Badge, Button, Card, CardAction, CardContent, CardHeader, CardTitle, Progress } from "@evinvest/uikit";
import { Fragment } from "react";

import { useT } from "@/shared/i18n";
import { formatShare } from "@/shared/lib/share";
import { useButtonSize } from "@/shared/ui/touch";

import type { PlaceRow } from "../model/rows";

/**
 * One location's mini funnel against its leads. A bar is a proportion, so it is
 * drawn only where a percent could be said too (spec §10.1); a small sample is
 * "n of m" and nothing more. A named location opens its site data.
 */
export function PlaceCard({ row, onOpen }: { row: PlaceRow; onOpen: (() => void) | null }) {
  const t = useT();
  const button = useButtonSize();
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="wrap-anywhere text-sm">
          {row.brand} · {row.location ?? t("places.unknown")}
        </CardTitle>
        {onOpen && (
          <CardAction>
            <Button variant="outline" size={button("xs")} onClick={onOpen}>
              {t("placeSettings.open")}
            </Button>
          </CardAction>
        )}
        {row.site && (row.site.hasSettings || row.site.withdrawn) && (
          <div className="flex flex-wrap gap-1">
            {row.site.hasSettings && <Badge variant="secondary">{t("placeSettings.badge.live")}</Badge>}
            {row.site.withdrawn && <Badge variant="destructive">{t("placeSettings.badge.withdrawn")}</Badge>}
          </div>
        )}
      </CardHeader>
      <CardContent className="flex flex-col gap-2 px-4">
        {/* One grid for every row, so the share column — "50%" or "3 of 4" — is as wide in each and the bars line up. */}
        <div className="grid grid-cols-(--grid-place-step) items-center gap-2 text-sm">
          <span className="text-ink-mid">{t("places.leads")}</span>
          <span />
          <span className="text-right tabular-nums text-ink">{row.leads}</span>
          {row.steps.map(({ stage, share }) => (
            <Fragment key={stage}>
              <span className="text-ink-mid">{t(`places.${stage}`)}</span>
              {share.small_sample || share.percent === null ? <span /> : <Progress value={share.percent} aria-label={t(`places.${stage}`)} />}
              <span className="text-right tabular-nums text-ink">{formatShare(share, t)}</span>
            </Fragment>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
