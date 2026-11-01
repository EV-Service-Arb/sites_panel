import { Button, toast } from "@evinvest/uikit";

import { type Lead, type Messenger, markMessaged, refOf } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { useKeyedWrite } from "@/shared/lib/use-keyed-write";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * The operator saw the customer's message (matched by its ref) in WhatsApp
 * Business or the bot's chat: `lead.messaged` from the panel. A retry after a
 * lost answer goes under the same Idempotency-Key, so the server acts once.
 */
export function MarkMessagedButton({ lead, channel, onMarked }: { lead: Lead; channel: Messenger; onMarked: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const { busy, run } = useKeyedWrite();

  const mark = async () => {
    const outcome = await run({ brand: lead.brand, lead: lead.lead_id, channel }, (key) => markMessaged(refOf(lead), channel, key));
    if (!outcome.ok) return notifyFailure(outcome.error, t);
    toast.positive(t("move.saved"));
    onMarked();
  };

  return (
    <Button size={button()} variant="outline" className="self-start" disabled={busy} onClick={() => void mark()}>
      {t("messenger.mark")}
    </Button>
  );
}
