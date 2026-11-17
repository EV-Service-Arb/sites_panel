import { useResource } from "@evinvest/api/react";
import { Settled, Skeleton } from "@evinvest/uikit";
import { useMemo, useState } from "react";

import { byLocation } from "@/entities/funnel";
import type { PlaceKey } from "@/entities/place";
import { useMe } from "@/entities/session";
import { AddPlaceButton } from "@/features/add-place";
import { type Period, rangeOf } from "@/features/funnel-filters";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { replaceQuery, useQuery } from "@/shared/lib/router";
import { useLastNonNull } from "@/shared/lib/use-last-non-null";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { ScreenFrame } from "@/shared/ui/screen-frame";

import { offeredBrands } from "../model/brands";
import { type PlaceRow, placeRows } from "../model/rows";
import { PlaceCard } from "./place-card";
import { PlaceSettingsPanel } from "./place-settings-panel";

const WINDOW_DAYS: Period = 30;

/**
 * Locations with a mini funnel each, from the backend's per-location funnel,
 * plus what `/places` says of their site data; a card opens that data.
 */
export function PlacesView() {
  const t = useT();
  const me = useMe();
  // Fixed when the screen opens: a range that moved mid-render would refetch in a loop.
  const [range] = useState(() => rangeOf(WINDOW_DAYS, new Date()));
  // The open place lives in the URL (`?brand=&location=`), so another service can link to it.
  const params = useQuery();
  const brand = params.get("brand");
  const location = params.get("location");
  const open = useMemo<PlaceKey | null>(() => (brand && location ? { brand, slug: location } : null), [brand, location]);
  const setOpen = (key: PlaceKey | null) => replaceQuery(key ? new URLSearchParams({ brand: key.brand, location: key.slug }) : new URLSearchParams());
  // Title and form stay put while the sheet slides away.
  const shown = useLastNonNull(open);
  const gmb = gmbParam(useGmbs());
  // The cards count leads, and say whether a place has live data or is withdrawn; each read follows its own tables.
  const funnel = useResource(api(), "/funnel", { params: { query: { ...range, gmb, by: "location" } } });
  const places = useResource(api(), "/places", { params: { query: { gmb } } });
  const pricing = useResource(api(), "/pricing");
  const refresh = () => {
    funnel.reload();
    places.reload();
  };
  const failure = [funnel, places, pricing].find((r) => r.status === "error")?.failure;
  const data =
    funnel.status === "ok" && places.status === "ok" && pricing.status === "ok"
      ? { rows: placeRows(byLocation(funnel.data), places.data.places), minSample: funnel.data.min_sample, brands: offeredBrands(places.data.places, pricing.data.items) }
      : null;

  return (
    <ScreenFrame
      title={t("places.title")}
      actions={
        api().mayCall("post", "/places", me.permissions) && data !== null ? (
          <AddPlaceButton
            brands={data.brands}
            onAdded={(key) => {
              refresh();
              setOpen(key);
            }}
          />
        ) : undefined
      }
    >
      <Settled loading={data === null && failure === undefined} skeleton={<Skeleton className="h-48 w-full" />} className="flex flex-col gap-4">
        {failure !== undefined && <ErrorState failure={failure} onRetry={refresh} />}
        {data !== null && <p className="text-sm text-ink-soft">{t("places.window", { days: WINDOW_DAYS, min: data.minSample })}</p>}
        {data !== null && <PlaceGrid rows={data.rows} onOpen={setOpen} />}
      </Settled>
      <PanelOverlay
        open={open !== null}
        onOpenChange={(o) => !o && setOpen(null)}
        desktop="sheet"
        title={shown ? t("placeSettings.openFor", { brand: shown.brand, slug: shown.slug }) : ""}
        description={t("placeSettings.description")}
      >
        {shown && <PlaceSettingsPanel key={`${shown.brand}/${shown.slug}`} place={shown} onChanged={refresh} />}
      </PanelOverlay>
    </ScreenFrame>
  );
}

function PlaceGrid({ rows, onOpen }: { rows: PlaceRow[]; onOpen: (key: PlaceKey) => void }) {
  const t = useT();
  if (rows.length === 0) return <EmptyState title={t("places.empty")} description={t("places.empty.body")} />;
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {rows.map((row) => {
        const slug = row.location;
        return <PlaceCard key={`${row.brand}/${slug ?? ""}`} row={row} onOpen={slug === null ? null : () => onOpen({ brand: row.brand, slug })} />;
      })}
    </div>
  );
}
