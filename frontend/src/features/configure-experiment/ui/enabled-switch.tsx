import { Switch } from "@evinvest/uikit";

import type { Experiment } from "@/entities/experiment";
import { useT } from "@/shared/i18n";

import { enabledPatch } from "../model/draft";
import { useConfigure } from "../model/use-configure";

/**
 * The kill switch, in the row itself: turning an experiment off is the one
 * change made in a hurry, so it is one click, not a dialog.
 */
export function EnabledSwitch({ experiment, onSaved }: { experiment: Experiment; onSaved: (updated: Experiment) => void }) {
  const t = useT();
  const { busy, save } = useConfigure(experiment, onSaved);
  const on = experiment.effective.enabled;
  return (
    <Switch
      checked={on}
      disabled={busy}
      aria-label={t("experiments.enabled.label", { key: experiment.key })}
      onCheckedChange={(next) => void save(enabledPatch(experiment, next), t(next ? "experiments.enabled.on" : "experiments.enabled.off", { key: experiment.key }))}
    />
  );
}
