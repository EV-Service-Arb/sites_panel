import { Button, toast } from "@evinvest/uikit";
import { useState } from "react";

import { type Lead, type StageMove, moveLead, refOf } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

import { MOVES, type MoveKind } from "../model/moves";
import { LostForm } from "./lost-form";
import { QuoteForm } from "./quote-form";

/**
 * One button per next step. A step that needs nothing more is saved on the tap;
 * a quote and a loss open their short form in place.
 */
export function StageActions({ lead, onMoved }: { lead: Lead; onMoved: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const [open, setOpen] = useState<"quoted" | "lost" | null>(null);
  const [busy, setBusy] = useState(false);
  const moves = MOVES[lead.stage];
  if (moves.length === 0) return null;

  const save = async (move: StageMove) => {
    setBusy(true);
    try {
      await moveLead(refOf(lead), move);
      toast.positive(t("move.saved"));
      setOpen(null);
      onMoved();
    } catch (e) {
      notifyFailure(e, t);
    } finally {
      setBusy(false);
    }
  };

  const tap = (kind: MoveKind) => {
    if (kind === "quoted" || kind === "lost") setOpen(kind);
    else void save(kind === "contacted" ? { stage: "contacted", channel: "phone" } : { stage: kind });
  };

  if (open === "quoted") return <QuoteForm busy={busy} onSubmit={save} onCancel={() => setOpen(null)} />;
  if (open === "lost") return <LostForm busy={busy} onSubmit={save} onCancel={() => setOpen(null)} />;

  return (
    <div className="flex flex-wrap gap-2">
      {moves.map((kind) => (
        <Button key={kind} size={button()} variant={kind === "lost" ? "ghost" : "outline"} disabled={busy} onClick={() => tap(kind)}>
          {t(`move.${kind}`)}
        </Button>
      ))}
    </div>
  );
}
