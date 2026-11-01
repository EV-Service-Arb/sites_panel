import { Badge, Table, TableBody, TableCell, TableHead, TableHeader, TableRow, cn } from "@evinvest/uikit";

import { BookingLine, ChannelBadge, DealSummary, type Lead, SlaBadge, StageBadge, SuspectBadge, contactOf } from "@/entities/lead";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { EDGE_CELL, TABLE_HEAD } from "@/shared/ui/table";

import { leadKey } from "../model/live-merge";
import type { Flash } from "../model/use-lead-list";
import { flashAttr } from "./flash";

const HEAD = cn(TABLE_HEAD, EDGE_CELL);

export interface LeadTableProps {
  leads: Lead[];
  flash: ReadonlyMap<string, Flash>;
  /** The open lead's key: its row wears the kit's selected fill. */
  selected: string | null;
  onOpen: (lead: Lead) => void;
}

/** Desktop: the queue as a table; a row opens the lead beside it. */
export function LeadTable({ leads, flash, selected, onOpen }: LeadTableProps) {
  const t = useT();
  const locale = useLocale();
  return (
    <Table>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead className={HEAD}>{t("leads.col.need")}</TableHead>
          <TableHead className={HEAD}>{t("leads.col.where")}</TableHead>
          <TableHead className={HEAD}>{t("leads.col.stage")}</TableHead>
          <TableHead className={HEAD}>{t("leads.col.booking")}</TableHead>
          <TableHead className={cn(HEAD, "text-right")}>{t("leads.col.price")}</TableHead>
          <TableHead className={HEAD}>{t("leads.col.waiting")}</TableHead>
          <TableHead className={cn(HEAD, "text-right")}>{t("leads.col.created")}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {leads.map((lead) => {
          const c = contactOf(lead.pii);
          const key = leadKey(lead);
          const isOpen = key === selected;
          return (
            <TableRow
              key={key}
              data-flash={flashAttr(flash.get(key))}
              data-state={isOpen ? "selected" : undefined}
              className="cursor-pointer"
              onClick={(e) => {
                // A click anywhere on the row lands on its button, so closing the card hands focus back here.
                e.currentTarget.querySelector("button")?.focus({ preventScroll: true });
                onOpen(lead);
              }}
            >
              <TableCell className={cn(EDGE_CELL, "max-w-72")}>
                {/* The row takes the click; the button is its keyboard stop. */}
                <button
                  type="button"
                  aria-pressed={isOpen}
                  className="block w-full truncate rounded-sm text-left font-medium text-ink outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  {c.need ?? c.name ?? t("leads.noNeed")}
                </button>
              </TableCell>
              <TableCell className={cn(EDGE_CELL, "text-ink-mid")}>
                <span className="flex flex-wrap items-center gap-1.5">
                  {lead.brand} · {lead.location ?? "—"}
                  <ChannelBadge channel={lead.channel} />
                  {lead.manual && <Badge variant="outline">{t("leads.manual")}</Badge>}
                </span>
              </TableCell>
              <TableCell className={EDGE_CELL}>
                <span className="flex flex-wrap items-center gap-1">
                  <StageBadge stage={lead.stage} />
                  <SuspectBadge suspect={lead.suspect} short />
                </span>
              </TableCell>
              {/* The slot wraps under its badge rather than pushing the last columns out of the card. */}
              <TableCell className={cn(EDGE_CELL, "max-w-56 whitespace-normal")}>
                <BookingLine booking={lead.booking} />
              </TableCell>
              <TableCell className={cn(EDGE_CELL, "[&>span]:justify-end")}>
                <DealSummary lead={lead} />
              </TableCell>
              <TableCell className={EDGE_CELL}>
                <SlaBadge sla={lead.sla} />
              </TableCell>
              <TableCell className={cn(EDGE_CELL, "text-right tabular-nums text-ink-soft")}>{lead.created_at ? formatDateTime(lead.created_at, locale) : "—"}</TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}
