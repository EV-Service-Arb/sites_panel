import { Alert, AlertDescription, Button, Card, CardContent, CardDescription, CardHeader, CardTitle } from "@evinvest/uikit";
import { useState } from "react";

import { type PricingModel, labelOf } from "@/entities/pricing";
import { useLocale, useT } from "@/shared/i18n";

import { answersFor, needFor } from "../model/answers";
import { usePreview } from "../model/use-preview";
import { AnswerSelect } from "./answer-select";
import { PreviewResult } from "./preview-result";

export interface PreviewPanelProps {
  brand: string;
  /** The model to price — the draft's, as it stands — or null while the draft does not make one. */
  model: PricingModel | null;
  /** Shows the field a model path names. */
  onShowPath: (path: string) => void;
  /** With no model: shows what keeps the draft from making one. */
  onShowProblem?: () => void;
  /** The card's DOM id: where a phone's "Calculate" in the editor's action bar takes the reader. */
  id?: string;
}

/** "Calculate": a need and the visitor's answers, priced by the server exactly as the site prices them. */
export function PreviewPanel({ brand, model, onShowPath, onShowProblem, id }: PreviewPanelProps) {
  const t = useT();
  const [pickedNeed, setPickedNeed] = useState<string | null>(null);
  const [picked, setPicked] = useState<Record<string, string>>({});
  const need = model ? needFor(model, pickedNeed) : null;
  const answers = model && need !== null ? answersFor(model, need, picked) : {};
  const state = usePreview(brand, model, need, answers);
  return (
    <Card id={id} tabIndex={-1} className="scroll-mt-4 outline-none">
      <CardHeader>
        <CardTitle>{t("pricing.preview.title")}</CardTitle>
        <CardDescription>{t("pricing.preview.body")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {!model && (
          <Alert>
            <AlertDescription className="flex flex-col items-start gap-2">
              {t("pricing.preview.noModel")}
              {onShowProblem && (
                <Button type="button" variant="link" className="px-0" onClick={onShowProblem}>
                  {t("pricing.preview.showField")}
                </Button>
              )}
            </AlertDescription>
          </Alert>
        )}
        {model && need === null && <p className="text-sm text-ink-soft">{t("pricing.preview.noNeeds")}</p>}
        {model && need !== null && (
          <Choices model={model} need={need} answers={answers} onNeed={setPickedNeed} onAnswer={(input, option) => setPicked((p) => ({ ...p, [input]: option }))} />
        )}
        <PreviewResult state={state} onShowPath={onShowPath} />
      </CardContent>
    </Card>
  );
}

interface ChoicesProps {
  model: PricingModel;
  need: string;
  answers: Record<string, string>;
  onNeed: (need: string) => void;
  onAnswer: (input: string, option: string) => void;
}

function Choices({ model, need, answers, onNeed, onAnswer }: ChoicesProps) {
  const t = useT();
  const locale = useLocale();
  return (
    <div className="flex flex-col gap-2">
      <AnswerSelect
        label={t("pricing.preview.need")}
        value={need}
        options={Object.entries(model.needs).map(([id, n]) => ({ value: id, label: `${id} · ${t(`pricing.needKind.${n.kind}`)}` }))}
        onChange={onNeed}
      />
      {Object.keys(answers).map((inputId) => {
        const input = model.inputs.find((i) => i.id === inputId);
        if (!input) return null;
        return (
          <AnswerSelect
            key={inputId}
            label={labelOf(input.labels, locale) || inputId}
            value={answers[inputId] ?? ""}
            options={input.options.map((o) => ({ value: o.id, label: labelOf(o.labels, locale) || o.id }))}
            onChange={(option) => onAnswer(inputId, option)}
          />
        );
      })}
    </div>
  );
}
