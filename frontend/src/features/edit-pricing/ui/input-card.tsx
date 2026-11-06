import { INPUT_KINDS, type PricingInputKind } from "@evinvest/sites-contract";
import { Button } from "@evinvest/uikit";
import { Plus } from "lucide-react";

import { labelOf } from "@/entities/pricing";
import { useLocale, useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import type { InputDraft } from "../model/draft";
import { FIELD, domIdOf } from "../model/fields";
import { addOption, removeInput, setInputKind, updateInput } from "../model/ops";
import type { PricingEditor } from "../model/use-pricing-editor";
import { EditorCard } from "./editor-card";
import { FieldMessages } from "./field-messages";
import { KindSelect } from "./kind-select";
import { LabelsFields } from "./labels-fields";
import { OptionRow } from "./option-row";
import { TextField } from "./text-field";

/** A question the visitor answers: its slug, what its answers do, its words, and the answers. */
export function InputCard({ editor, input }: { editor: PricingEditor; input: InputDraft }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  const { errors } = editor;
  const title = labelOf(input.labels, locale).trim() || input.id || t("pricing.input.untitled");
  return (
    <EditorCard id={domIdOf(FIELD.input(input.key))} title={title} removeLabel={t("pricing.input.remove")} onRemove={() => editor.update((d) => removeInput(d, input.key))}>
      <FieldMessages shown={errors.byField.get(FIELD.input(input.key))} />
      <div className="grid items-start gap-2 sm:grid-cols-2">
        <TextField field={FIELD.inputId(input.key)} label={t("pricing.field.inputId")} context={title} hint={t("pricing.hint.slug")} kind="slug" value={input.id} onChange={(id) => editor.update((d) => updateInput(d, input.key, { id }))} errors={errors} />
        <KindSelect<PricingInputKind>
          field={FIELD.inputKind(input.key)}
          label={t("pricing.field.kind")}
          context={title}
          value={input.kind}
          options={INPUT_KINDS.map((k) => ({ value: k, label: t(`pricing.kind.${k}`) }))}
          onChange={(kind) => editor.update((d) => setInputKind(d, input.key, kind))}
          errors={errors}
        />
      </div>
      <LabelsFields labels={input.labels} locales={editor.base.locales} field={FIELD.inputLabels(input.key)} onChange={(labels) => editor.update((d) => updateInput(d, input.key, { labels }))} errors={errors} context={title} />
      <section id={domIdOf(FIELD.inputOptions(input.key))} tabIndex={-1} className="flex flex-col gap-2 outline-none" aria-label={t("pricing.options.title")}>
        <h4 className="text-sm font-medium text-ink">{t("pricing.options.title")}</h4>
        <FieldMessages shown={errors.byField.get(FIELD.inputOptions(input.key))} />
        <ul className="flex flex-col gap-3">
          {input.options.map((o, i) => (
            <OptionRow key={o.key} editor={editor} input={input} option={o} index={i} />
          ))}
        </ul>
        <Button type="button" variant="outline" size={button("sm")} className="self-start" onClick={() => editor.update((d) => addOption(d, input.key))}>
          <Plus aria-hidden className="size-4" />
          {t("pricing.option.add")}
        </Button>
      </section>
    </EditorCard>
  );
}
