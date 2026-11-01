import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@evinvest/uikit";

import { BookingLine, ChannelBadge, DealSummary, type Lead, SlaBadge, StageBadge, SuspectBadge, contactOf } from "@/entities/lead";
import { useT } from "@/shared/i18n";

import { leadKey } from "../model/live-merge";
import type { Flash } from "../model/use-lead-list";
import { flashAttr } from "./flash";

/** Phone: the queue as rows split by hairlines in the same card as the table; a tap opens the lead in a bottom sheet. */
export function LeadList({ leads, flash, selected, onOpen }: { leads: Lead[]; flash: ReadonlyMap<string, Flash>; selected: string | null; onOpen: (lead: Lead) => void }) {
  const t = useT();
  return (
    <ItemGroup className="divide-y divide-border">
      {leads.map((lead) => {
        const c = contactOf(lead.pii);
        return (
          <Item key={leadKey(lead)} data-flash={flashAttr(flash.get(leadKey(lead)))} size="sm" className="rounded-none" asChild>
            <button type="button" aria-pressed={leadKey(lead) === selected} className="w-full text-left active:bg-muted/50 aria-pressed:bg-muted" onClick={() => onOpen(lead)}>
              <ItemContent className="min-w-0">
                <ItemTitle className="w-full truncate">{c.need ?? c.name ?? t("leads.noNeed")}</ItemTitle>
                <ItemDescription>
                  {lead.brand} · {lead.location ?? "—"}
                  {lead.manual && ` · ${t("leads.manual")}`}
                </ItemDescription>
                <DealSummary lead={lead} />
                {lead.booking.status !== "none" && <BookingLine booking={lead.booking} />}
              </ItemContent>
              <ItemActions className="flex-col items-end gap-1">
                {lead.sla ? <SlaBadge sla={lead.sla} /> : <StageBadge stage={lead.stage} />}
                <SuspectBadge suspect={lead.suspect} short />
                <ChannelBadge channel={lead.channel} short />
              </ItemActions>
            </button>
          </Item>
        );
      })}
    </ItemGroup>
  );
}
