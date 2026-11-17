import { useResource } from "@evinvest/api/react";
import { Button } from "@evinvest/uikit";
import { useCallback, useState } from "react";

import { type LeadFilter, type LeadRef, decodeRef, encodeRef } from "@/entities/lead";
import { brandsOf, usePlaces } from "@/entities/place";
import { CallFlowProvider, OutcomeSheet } from "@/features/call-lead";
import { CreateLeadButton } from "@/features/create-lead";
import { LeadFilters, leadFilterFrom, narrows, paramsWith } from "@/features/lead-filters";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { gmbParam, useGmbs } from "@/shared/lib/gmbs";
import { replaceQuery, useQuery } from "@/shared/lib/router";
import { useLastNonNull } from "@/shared/lib/use-last-non-null";
import { useRereadOn } from "@/shared/lib/use-reread-on";
import { EmptyState } from "@/shared/ui/empty-state";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { ScreenFrame } from "@/shared/ui/screen-frame";
import { useButtonSize } from "@/shared/ui/touch";

import { LeadCardPanel } from "./lead-card-panel";
import { LeadQueue } from "./lead-queue";
import { UnmatchedBookings } from "./unmatched-bookings";

export function LeadsView() {
  const t = useT();
  const button = useButtonSize();
  const params = useQuery();
  const gmbs = useGmbs();
  const filter = leadFilterFrom(params);
  const open = decodeRef(params.get("lead"));
  // The card stays filled while the sheet slides away; the param is already gone by then.
  const shown = decodeRef(useLastNonNull(params.get("lead")));
  const [version, setVersion] = useState(0);

  const setFilter = (patch: Partial<LeadFilter>) => replaceQuery(paramsWith(params, patch));
  const openLead = (ref: LeadRef | null) => {
    const next = new URLSearchParams(params);
    if (ref) next.set("lead", encodeRef(ref));
    else next.delete("lead");
    replaceQuery(next);
  };
  const changed = useCallback(() => setVersion((v) => v + 1), []);
  const created = (ref: LeadRef) => {
    changed();
    openLead(ref);
  };
  const filtered = narrows(filter);
  const counts = useResource(api(), "/leads/counts", { params: { query: { brand: filter.brand ?? null, gmb: gmbParam(gmbs) } } });
  useRereadOn(version, counts.reload);

  const places = usePlaces(version);
  const brands = brandsOf(places, filter.brand);
  const create = <CreateLeadButton brands={brands} places={places} onCreated={created} />;

  return (
    <CallFlowProvider onLogged={changed}>
      <ScreenFrame title={t("leads.title")} actions={create}>
        <LeadFilters filter={filter} brands={brands} counts={counts.status === "ok" ? counts.data : null} onChange={setFilter} />
        <UnmatchedBookings brand={filter.brand ?? null} version={version} onAttached={changed} />
        <LeadQueue
          filter={filter}
          gmbs={gmbs}
          version={version}
          selected={open}
          onOpen={openLead}
          empty={
            <EmptyState title={t("leads.empty")} description={t(filtered ? "leads.empty.filtered" : gmbs !== null ? "leads.empty.selection" : "leads.empty.none")}>
              {filtered && (
                <Button variant="outline" size={button()} onClick={() => replaceQuery(new URLSearchParams())}>
                  {t("leads.resetFilters")}
                </Button>
              )}
              {create}
            </EmptyState>
          }
        />
      </ScreenFrame>
      <PanelOverlay open={open !== null} onOpenChange={(o) => !o && openLead(null)} title={t("card.title")} desktop="sheet">
        {/* Keyed by lead, not the overlay: another lead in the open sheet starts a fresh card (what it has
            seen of the last one is not "changed elsewhere"), and the sheet itself stays mounted. */}
        {shown && <LeadCardPanel key={encodeRef(shown)} leadRef={shown} version={version} onChanged={changed} />}
      </PanelOverlay>
      <OutcomeSheet />
    </CallFlowProvider>
  );
}
