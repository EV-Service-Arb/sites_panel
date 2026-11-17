import { Button, toast } from "@evinvest/uikit";
import { useState } from "react";

import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { useButtonSize } from "@/shared/ui/touch";

import { type CallOutcome, callOutcomeValues } from "@/shared/api/schema";

import { useCallFlow } from "../model/provider";

/** "How did the call end?" — the bottom sheet that opens on coming back to the tab. */
export function OutcomeSheet({ subtitle }: { subtitle?: string }) {
  const t = useT();
  const button = useButtonSize();
  const { state, answer, dismiss } = useCallFlow();
  const [busy, setBusy] = useState(false);

  const pick = async (outcome: CallOutcome) => {
    setBusy(true);
    const refused = await answer(outcome);
    setBusy(false);
    if (refused === null) toast.positive(t("call.saved"));
    else notifyFailure(refused, t);
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
        {callOutcomeValues.map((outcome, i) => (
          <Button key={outcome} size={button("lg")} variant={i === 0 ? "primary" : "outline"} disabled={busy} onClick={() => pick(outcome)}>
            {t(`call.outcome.${outcome}`)}
          </Button>
        ))}
      </div>
    </PanelOverlay>
  );
}
