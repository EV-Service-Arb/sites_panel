import { PRICING_LIMITS } from "@evinvest/sites-contract";
import { Button } from "@evinvest/uikit";
import { ArrowDown, ArrowUp, Plus, X } from "lucide-react";

import { labelOf } from "@/entities/pricing";
import { useLocale, useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import type { InputDraft, NeedDraft } from "../model/draft";
import { FIELD, domIdOf } from "../model/fields";
import { moveNeedInput, toggleNeedInput } from "../model/ops";
import type { PricingEditor } from "../model/use-pricing-editor";
import { FieldMessages } from "./field-messages";

/** The questions an estimate asks, in the order the form asks them; at most twelve (what a lead carries). */
export function AskedInputs({ editor, need }: { editor: PricingEditor; need: NeedDraft }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  const nameOf = (i: InputDraft) => labelOf(i.labels, locale).trim() || i.id || t("pricing.input.untitled");
  const asked = need.inputs.map((k) => editor.draft.inputs.find((i) => i.key === k)).filter((i): i is InputDraft => i !== undefined);
  const rest = editor.draft.inputs.filter((i) => !need.inputs.includes(i.key));
  const full = asked.length >= PRICING_LIMITS.maxNeedInputs;
  const field = FIELD.needInputs(need.key);
  return (
    <div id={domIdOf(field)} tabIndex={-1} className="flex flex-col gap-2 outline-none">
      <span className="text-sm font-medium text-ink">{t("pricing.need.asked", { n: asked.length, max: PRICING_LIMITS.maxNeedInputs })}</span>
      {asked.length === 0 && <p className="text-sm text-ink-soft">{t("pricing.need.askedNone")}</p>}
      <ol className="flex flex-col gap-1">
        {asked.map((input, i) => (
          <li key={input.key} className="flex items-center gap-2 rounded-md border border-border px-2 py-1 text-sm text-ink">
            <span className="w-5 text-ink-soft">{i + 1}.</span>
            <span className="min-w-0 flex-1 truncate">{nameOf(input)}</span>
            <Button type="button" variant="ghost" size={button("sm")} icon aria-label={t("pricing.need.up")} disabled={i === 0} onClick={() => editor.update((d) => moveNeedInput(d, need.key, input.key, -1))}>
              <ArrowUp aria-hidden className="size-4" />
            </Button>
            <Button type="button" variant="ghost" size={button("sm")} icon aria-label={t("pricing.need.down")} disabled={i === asked.length - 1} onClick={() => editor.update((d) => moveNeedInput(d, need.key, input.key, 1))}>
              <ArrowDown aria-hidden className="size-4" />
            </Button>
            <Button type="button" variant="ghost" size={button("sm")} icon aria-label={t("pricing.need.unask", { name: nameOf(input) })} onClick={() => editor.update((d) => toggleNeedInput(d, need.key, input.key))}>
              <X aria-hidden className="size-4" />
            </Button>
          </li>
        ))}
      </ol>
      {rest.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {rest.map((input) => (
            <Button key={input.key} type="button" variant="outline" size={button("sm")} disabled={full} onClick={() => editor.update((d) => toggleNeedInput(d, need.key, input.key))}>
              <Plus aria-hidden className="size-4" />
              {nameOf(input)}
            </Button>
          ))}
        </div>
      )}
      <FieldMessages shown={editor.errors.byField.get(field)} />
    </div>
  );
}
