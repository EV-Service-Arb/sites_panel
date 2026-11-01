import { Button, toast } from "@evinvest/uikit";
import { Copy } from "lucide-react";

import { ChannelIcon, type Lead, awaitingMessage } from "@/entities/lead";
import { MarkMessagedButton } from "@/features/mark-messaged";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * The messenger side of a lead: the ref the customer quotes ("Réf. AQ-7K3F",
 * as the prefilled message ends), and whether they have written yet. Nothing
 * for a lead with neither a ref nor a messenger channel.
 */
export function MessengerBlock({ lead, onChanged }: { lead: Lead; onChanged: () => void }) {
  const t = useT();
  const locale = useLocale();
  const awaiting = awaitingMessage(lead);
  const messaged = lead.messaged_at !== null ? { at: formatDateTime(lead.messaged_at, locale), channel: lead.messaged_channel } : null;
  if (lead.message_ref === null && awaiting === null && messaged === null) return null;

  return (
    <div className="flex flex-col gap-2">
      {lead.message_ref !== null && <RefLine messageRef={lead.message_ref} />}
      {messaged && (
        <p className="flex items-center gap-1.5 text-sm text-ink-mid">
          <ChannelIcon channel={messaged.channel ?? "other"} className="size-4 shrink-0" />
          {messaged.channel ? t("messenger.done", { channel: t(`channel.${messaged.channel}`), at: messaged.at }) : t("messenger.doneOther", { at: messaged.at })}
        </p>
      )}
      {awaiting && (
        <>
          <p className="text-sm text-ink-soft">{t("messenger.awaiting")}</p>
          <MarkMessagedButton lead={lead} channel={awaiting} onMarked={onChanged} />
        </>
      )}
    </div>
  );
}

function RefLine({ messageRef }: { messageRef: string }) {
  const t = useT();
  const button = useButtonSize();
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(messageRef);
      toast.positive(t("messenger.ref.copied"));
    } catch {
      // No clipboard outside a secure context: the code stays selectable by hand.
      toast.error(t("messenger.ref.copyFailed"));
    }
  };
  return (
    <p className="flex items-center gap-2 text-sm">
      <span className="text-ink-soft">{t("messenger.ref")}</span>
      <code className="font-mono text-ink select-all">{messageRef}</code>
      <Button type="button" variant="ghost" size={button("xs")} aria-label={t("messenger.ref.copy")} onClick={() => void copy()}>
        <Copy aria-hidden />
      </Button>
    </p>
  );
}
