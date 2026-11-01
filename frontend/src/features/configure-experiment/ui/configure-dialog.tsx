import { Button, Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@evinvest/uikit";
import { useState } from "react";

import type { Experiment } from "@/entities/experiment";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import { ConfigureForm } from "./configure-form";

/**
 * The weights and the holdout, in a dialog: unlike the switch they take some
 * typing. The form mounts on open, so it starts from the experiment as it is
 * then, and a live re-read while it is open does not wipe what is typed.
 */
export function ConfigureDialog({ experiment, onSaved }: { experiment: Experiment; onSaved: (updated: Experiment) => void }) {
  const t = useT();
  const button = useButtonSize();
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" size={button("sm")} aria-label={t("experiments.edit.label", { key: experiment.key })}>
          {t("experiments.edit")}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="break-all">{t("experiments.edit.title", { key: experiment.key, brand: experiment.brand })}</DialogTitle>
          <DialogDescription>{t("experiments.edit.body")}</DialogDescription>
        </DialogHeader>
        {open && <ConfigureForm experiment={experiment} onSaved={onSaved} onDone={() => setOpen(false)} />}
      </DialogContent>
    </Dialog>
  );
}
