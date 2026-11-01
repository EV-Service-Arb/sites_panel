import { Badge, Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList, Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import type { UnmatchedBooking } from "@/entities/booking";
import { type LeadFilter, contactOf, fetchLeads } from "@/entities/lead";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { formatSlot } from "@/shared/lib/instant";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";
import { PanelOverlay } from "@/shared/ui/panel-overlay";

import { candidateText, rankCandidates, typedId } from "../model/candidates";
import { useAttach } from "../model/use-attach";

/** How many of the brand's newest leads are offered; an older one is attached by its id. */
const RECENT = 50;

const brandOnly = (brand: string): LeadFilter => ({ stage: null, brand, location: null, overdue: false, createdFrom: null, createdTo: null, suspect: null, flow: null, booking: null, channel: null, messageRef: null });

/**
 * Pick the lead a provider's booking belongs to: the brand's newest leads,
 * those with the booking's contact first, searched by any word; or a typed id.
 */
export function AttachDialog({ booking, onClose, onAttached }: { booking: UnmatchedBooking | null; onClose: () => void; onAttached: () => void }) {
  const t = useT();
  const locale = useLocale();
  return (
    <PanelOverlay
      open={booking !== null}
      onOpenChange={(open) => !open && onClose()}
      desktop="dialog"
      title={t("attach.title")}
      description={booking ? t("attach.description", { when: formatSlot(booking.start_at, booking.end_at, locale), brand: booking.brand }) : undefined}
    >
      {booking && <Picker key={booking.id} booking={booking} onAttached={onAttached} />}
    </PanelOverlay>
  );
}

function Picker({ booking, onAttached }: { booking: UnmatchedBooking; onAttached: () => void }) {
  const t = useT();
  const locale = useLocale();
  const [search, setSearch] = useState("");
  const { busy, attach } = useAttach(booking, onAttached);
  const leads = useResource(`attach:${booking.brand}`, () => fetchLeads(brandOnly(booking.brand), null, RECENT));
  const id = typedId(search);
  const pick = (lead: string) => !busy && void attach(lead);

  return (
    <Command search={search} onSearchChange={setSearch} className="rounded-md border border-border">
      <CommandInput placeholder={t("attach.search")} aria-label={t("attach.search")} />
      <CommandList className="max-h-80">
        {leads.status === "loading" && <Skeleton className="m-2 h-24" />}
        {leads.status === "error" && <ErrorState failure={leads.failure} onRetry={leads.reload} />}
        {id && (
          <CommandGroup heading={t("attach.typed")}>
            <CommandItem value={search} disabled={busy} onSelect={() => pick(id)}>
              {t("attach.byId", { id })}
            </CommandItem>
          </CommandGroup>
        )}
        {leads.status === "ok" && (
          <CommandGroup heading={t("attach.recent", { brand: booking.brand })}>
            {rankCandidates(leads.data.leads, booking.contact).map(({ lead, same }) => {
              const c = contactOf(lead.pii);
              return (
                <CommandItem key={lead.lead_id} value={candidateText(lead)} disabled={busy} onSelect={() => pick(lead.lead_id)} className="flex-col items-start gap-0.5">
                  <span className="flex w-full items-center gap-2">
                    <span className="truncate font-medium">{c.need ?? c.name ?? lead.lead_id}</span>
                    {same && <Badge variant="success">{t("attach.sameContact")}</Badge>}
                  </span>
                  <span className="text-xs text-ink-soft">
                    {[c.name, c.phone, lead.location, lead.created_at && formatDateTime(lead.created_at, locale)].filter(Boolean).join(" · ")}
                  </span>
                </CommandItem>
              );
            })}
          </CommandGroup>
        )}
        <CommandEmpty>{t("attach.empty")}</CommandEmpty>
      </CommandList>
    </Command>
  );
}
