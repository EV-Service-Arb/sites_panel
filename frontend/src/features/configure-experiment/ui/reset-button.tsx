import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
  Button,
  buttonVariants,
} from "@evinvest/uikit";

import type { Experiment } from "@/entities/experiment";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import { RESET_PATCH, resetMovesWeights } from "../model/draft";
import { useConfigure } from "../model/use-configure";

/** Every override dropped: the switch, the weights and the holdout go back to what the landing's code declares. */
export function ResetButton({ experiment, onSaved, onDone }: { experiment: Experiment; onSaved: (updated: Experiment) => void; onDone: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const { busy, save } = useConfigure(experiment, onSaved);
  const reset = async () => {
    if (await save(RESET_PATCH, t("experiments.reset.done", { key: experiment.key }))) onDone();
  };
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button type="button" variant="ghost" size={button()} disabled={busy} className="sm:mr-auto">
          {t("experiments.reset")}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("experiments.reset.title", { key: experiment.key })}</AlertDialogTitle>
          <AlertDialogDescription>
            {t("experiments.reset.body")}
            {resetMovesWeights(experiment) && ` ${t("experiments.edit.weightsWarning.body")}`}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className={buttonVariants({ variant: "outline", size: button() })}>{t("move.cancel")}</AlertDialogCancel>
          <AlertDialogAction className={buttonVariants({ variant: "primary", size: button() })} onClick={() => void reset()}>
            {t("experiments.reset.confirm")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
