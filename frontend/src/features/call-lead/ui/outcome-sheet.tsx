import { Button, toast } from "@evinvest/uikit";
import { useState } from "react";

import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { useButtonSize } from "@/shared/ui/touch";

import { CALL_OUTCOMES } from "../model/call-flow";
import { useCallFlow } from "../model/provider";

/** "How did the call end?" — the bottom sheet that opens on coming back to the tab. */
export function OutcomeSheet({ subtitle }: { subtitle?: string }) {
  const t = useT();
  const button = useButtonSize();
  const { state, answer, dismiss } = useCallFlow();
  const [busy, setBusy] = useState(false);

  const pick = async (outcome: (typeof CALL_OUTCOMES)[number]) => {
    setBusy(true);
    try {
      await answer(outcome);
      toast.positive(t("call.saved"));
    } catch (e) {
      notifyFailure(e, t);
    } finally {
      setBusy(false);
    }
  };

  return (
    <PanelOverlay
      open={state.phase === "asking"}
      onOpenChange={(open) => !open && dismiss()}
      title={t("call.question")}
      {...(subtitle ? { description: subtitle } : {})}
      desktop="dialog"
    >
      <div className="grid grid-cols-2 gap-2">
        {CALL_OUTCOMES.map((outcome, i) => (
          <Button key={outcome} size={button("lg")} variant={i === 0 ? "primary" : "outline"} disabled={busy} onClick={() => pick(outcome)}>
            {t(`call.outcome.${outcome}`)}
          </Button>
        ))}
      </div>
    </PanelOverlay>
  );
}
