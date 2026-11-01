import { Alert, AlertDescription, Button } from "@evinvest/uikit";
import { type ReactNode, useEffect } from "react";

import type { PricingItem } from "@/entities/pricing";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import { FIELD } from "../model/fields";
import { addInput, addNeed } from "../model/ops";
import type { PricingEditor } from "../model/use-pricing-editor";
import { ConflictAlert, FresherAlert } from "./conflict-alerts";
import { GeneralFields } from "./general-fields";
import { InputCard } from "./input-card";
import { ListSection } from "./list-section";
import { NeedCard } from "./need-card";
import { shownText } from "./shown-text";

export interface PricingEditorFormProps {
  editor: PricingEditor;
  /** The brand's pricing as saved by someone else since the draft started; null while nobody has. */
  fresher: PricingItem | null;
  /**
   * Start the draft again from this pricing, dropping its edits. `chosen`: the
   * person asked for it (a button), so the rebuilt screen puts their focus back
   * somewhere; an untouched draft following a save by someone else does not.
   */
  onTakeFresh: (item: PricingItem, chosen: boolean) => void;
  /** After a 409 that did not say what is current: read the brand again. */
  onReload: () => void;
  /** Beside Save in the action bar, where the screen puts more than the editor (a way to the preview). */
  extraAction?: ReactNode;
}

/**
 * The admin's editor of one brand's model. Someone else's save arriving live
 * never replaces what is typed: with no edits the draft takes it, with edits
 * it says so and the person decides.
 */
export function PricingEditorForm({ editor, fresher, onTakeFresh, onReload, extraAction }: PricingEditorFormProps) {
  const t = useT();
  const button = useButtonSize();
  const { state, changed, errors, draft } = editor;
  useEffect(() => {
    if (fresher && !changed) onTakeFresh(fresher, false);
  }, [fresher, changed, onTakeFresh]);
  return (
    <form
      className="flex flex-col gap-6"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        editor.save();
      }}
    >
      {state.kind === "conflict" && (
        <ConflictAlert
          current={state.current}
          onTakeFresh={() => (state.current ? onTakeFresh(state.current, true) : onReload())}
          onOverwrite={editor.overwrite}
        />
      )}
      {fresher && changed && state.kind !== "conflict" && <FresherAlert fresher={fresher} onTakeFresh={() => onTakeFresh(fresher, true)} />}
      {errors.general.length > 0 && (
        <Alert variant="destructive" role="alert">
          <AlertDescription className="flex flex-col gap-0.5">
            {errors.general.map((s, i) => (
              <span key={i} className="wrap-anywhere">
                {shownText(s, t)}
              </span>
            ))}
          </AlertDescription>
        </Alert>
      )}
      <GeneralFields editor={editor} />
      <ListSection
        field={FIELD.inputs}
        title={t("pricing.inputs.title")}
        description={t("pricing.inputs.body")}
        empty={draft.inputs.length === 0 ? t("pricing.inputs.empty") : null}
        shown={errors.byField.get(FIELD.inputs)}
        add={{ label: t("pricing.input.add"), onAdd: () => editor.update(addInput) }}
      >
        {draft.inputs.map((input) => (
          <InputCard key={input.key} editor={editor} input={input} />
        ))}
      </ListSection>
      <ListSection
        field={FIELD.needs}
        title={t("pricing.needs.title")}
        description={t("pricing.needs.body")}
        empty={draft.needs.length === 0 ? t("pricing.needs.empty") : null}
        shown={errors.byField.get(FIELD.needs)}
        add={{ label: t("pricing.need.add"), onAdd: () => editor.update(addNeed) }}
      >
        {draft.needs.map((need) => (
          <NeedCard key={need.key} editor={editor} need={need} />
        ))}
      </ListSection>
      {/* Sticky on every width: a phone's form runs to thousands of pixels. There it
          stops above the kit's fixed tab bar, which itself clears the home indicator. */}
      <div className="sticky bottom-[calc(var(--shell-tab-bar-h)+env(safe-area-inset-bottom,0px))] z-10 flex flex-wrap items-center gap-2 border-t border-border bg-background py-3 md:bottom-0">
        <Button type="submit" size={button()} disabled={!changed || state.kind === "saving" || state.kind === "conflict"}>
          {state.kind === "saving" ? t("pricing.saving") : t("pricing.save")}
        </Button>
        <Button type="button" variant="ghost" size={button()} disabled={!changed || state.kind === "saving"} onClick={editor.reset}>
          {t("pricing.reset")}
        </Button>
        <span className="text-sm text-ink-soft max-md:sr-only" aria-live="polite">
          {changed ? t("pricing.unsaved") : t("pricing.upToDate")}
        </span>
        {extraAction}
      </div>
    </form>
  );
}
