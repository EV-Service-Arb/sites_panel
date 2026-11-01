import { Alert, AlertDescription, AlertTitle, Button, DialogClose, DialogFooter, Field, FieldDescription, FieldError, FieldLabel, Input } from "@evinvest/uikit";
import { useId, useState } from "react";

import type { Experiment } from "@/entities/experiment";
import { useT } from "@/shared/i18n";
import { useButtonSize, useControlSize } from "@/shared/ui/touch";

import { checkDraft, draftOf } from "../model/draft";
import { useConfigure } from "../model/use-configure";
import { ResetButton } from "./reset-button";
import { WeightsFields } from "./weights-fields";

/** Weights and holdout, checked as a landing would apply them before anything is sent. */
export function ConfigureForm({ experiment, onSaved, onDone }: { experiment: Experiment; onSaved: (updated: Experiment) => void; onDone: () => void }) {
  const t = useT();
  const id = useId();
  const size = useControlSize();
  const button = useButtonSize();
  const [draft, setDraft] = useState(() => draftOf(experiment));
  const { busy, save } = useConfigure(experiment, onSaved);
  const check = checkDraft(experiment, draft);
  const errors = check.kind === "invalid" ? check.errors : {};

  const submit = async () => {
    if (check.kind !== "ready") return;
    if (await save(check.patch, t("experiments.edit.saved", { key: experiment.key }))) onDone();
  };

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <WeightsFields variants={experiment.variants} weights={draft.weights} invalid={errors.weights === true} onChange={(weights) => setDraft({ ...draft, weights })} />
      <Field className="flex flex-col gap-1" data-invalid={errors.holdout ? true : undefined}>
        <FieldLabel htmlFor={`${id}-holdout`}>{t("experiments.edit.holdout")}</FieldLabel>
        <Input
          id={`${id}-holdout`}
          size={size}
          inputMode="decimal"
          autoComplete="off"
          className="w-24 text-right tabular-nums"
          value={draft.holdout}
          aria-invalid={errors.holdout ? true : undefined}
          onChange={(e) => setDraft({ ...draft, holdout: e.target.value })}
        />
        <FieldDescription>{t("experiments.edit.holdout.hint")}</FieldDescription>
        {errors.holdout && <FieldError>{t("experiments.edit.holdout.invalid")}</FieldError>}
      </Field>
      {check.kind === "ready" && check.weightsChanged && <WeightsWarning />}
      <DialogFooter className="gap-2">
        {experiment.override !== null && <ResetButton experiment={experiment} onSaved={onSaved} onDone={onDone} />}
        <DialogClose asChild>
          <Button type="button" variant="outline" size={button()}>
            {t("move.cancel")}
          </Button>
        </DialogClose>
        <Button type="submit" size={button()} disabled={busy || check.kind !== "ready"}>
          {t("experiments.edit.save")}
        </Button>
      </DialogFooter>
    </form>
  );
}

/** A new split makes the arms' earlier numbers incomparable with the later ones: PostHog must be read from the change on. */
export function WeightsWarning() {
  const t = useT();
  return (
    <Alert variant="info" role="note">
      <AlertTitle>{t("experiments.edit.weightsWarning.title")}</AlertTitle>
      <AlertDescription>{t("experiments.edit.weightsWarning.body")}</AlertDescription>
    </Alert>
  );
}
