import { Badge } from "@evinvest/uikit";

import { ChannelBadge, type Lead, SlaBadge, StageBadge, SuspectBadge, contactOf, extrasOf } from "@/entities/lead";
import { lostReasonLabel } from "@/features/move-stage";
import { useT } from "@/shared/i18n";

import { FactList } from "./card-section";
import { MessengerBlock } from "./messenger-block";

/** Who and what: the customer as they left it, where, how they reached us, and how long they have waited. */
export function LeadDetails({ lead, onChanged }: { lead: Lead; onChanged: () => void }) {
  const t = useT();
  const c = contactOf(lead.pii);
  const rows = [
    ["card.need", c.need],
    ["card.name", c.name],
    ["card.phone", c.phone],
    ["card.email", c.email],
  ] as const;
  const extras = extrasOf(lead.pii);
  // Text only, as React renders it: every value here is the customer's own input.
  const shown: { label: string; value: string; mono?: boolean }[] = [
    ...rows.flatMap(([key, value]) => (value === null ? [] : [{ label: t(key), value }])),
    ...extras.labelled.map(({ key, value }) => ({ label: t(`card.pii.${key}`), value })),
    ...extras.other.map(({ key, value }) => ({ label: key, value, mono: true })),
  ];

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <StageBadge stage={lead.stage} />
        <SlaBadge sla={lead.sla} />
        <SuspectBadge suspect={lead.suspect} />
        <ChannelBadge channel={lead.channel} />
        {lead.manual && <Badge variant="outline">{t("leads.manual")}</Badge>}
        <span className="text-sm text-ink-soft">
          {lead.brand} · {lead.location ?? t("places.unknown")}
        </span>
      </div>
      <MessengerBlock lead={lead} onChanged={onChanged} />
      {lead.lost_reason && <p className="text-sm text-ink-mid">{t("card.lostReason", { reason: lostReasonLabel(lead.lost_reason, t) })}</p>}
      {shown.length === 0 ? (
        <p className="text-sm text-ink-soft">{t("card.noPii")}</p>
      ) : (
        <FactList rows={shown} />
      )}
    </div>
  );
}
