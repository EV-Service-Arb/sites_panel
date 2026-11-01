import { toast } from "@evinvest/uikit";
import { useState } from "react";

import { type Experiment, type ExperimentPatch, configureExperiment } from "@/entities/experiment";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

/**
 * One PUT at a time. Nothing more is needed for the backend's fresh-session
 * check (admin, as for `POST /sources`): it asks concierge again on the write
 * itself, a lost grant comes back as 403 and a gone session as 401 (off to
 * sign-in), both told the same way as every other write.
 */
export function useConfigure(experiment: Experiment, onSaved: (updated: Experiment) => void) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const save = async (patch: ExperimentPatch, done: string): Promise<boolean> => {
    setBusy(true);
    try {
      onSaved(await configureExperiment(experiment.brand, experiment.key, patch));
      toast.positive(done);
      return true;
    } catch (e) {
      notifyFailure(e, t);
      return false;
    } finally {
      setBusy(false);
    }
  };
  return { busy, save };
}
