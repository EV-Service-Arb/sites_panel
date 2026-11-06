import type { PricingModel, PricingProblem } from "@evinvest/sites-contract";
import { Alert, AlertDescription, Button, Field, FieldDescription, FieldLabel, Textarea, toast } from "@evinvest/uikit";
import { ClipboardCopy, ClipboardPaste } from "lucide-react";
import { useId, useState } from "react";

import { useT } from "@/shared/i18n";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { useButtonSize } from "@/shared/ui/touch";

import { PASTE_FIELD_CLASS, PROBLEMS_CLASS } from "../config/paste";
import { exportModel, importModel } from "../model/import";
import type { PricingEditor } from "../model/use-pricing-editor";
import { shownText } from "./shown-text";

const SHOWN_PROBLEMS = 8;

/**
 * The model as JSON, out and in: how a site's baked model comes into the
 * panel, and how the panel's goes back into a site's repository.
 */
export function JsonTransfer({ editor }: { editor: PricingEditor }) {
  const t = useT();
  const button = useButtonSize();
  const [open, setOpen] = useState(false);
  const model = editor.serialized.model;
  const copy = async () => {
    if (!model) return;
    try {
      await navigator.clipboard.writeText(exportModel(model));
      toast.positive(t("pricing.json.copied"));
    } catch {
      toast.error(t("pricing.json.copyFailed"));
    }
  };
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" size={button("sm")} disabled={!model} onClick={() => void copy()}>
          <ClipboardCopy aria-hidden className="size-4" />
          {t("pricing.json.copy")}
        </Button>
        <Button type="button" variant="outline" size={button("sm")} onClick={() => setOpen(true)}>
          <ClipboardPaste aria-hidden className="size-4" />
          {t("pricing.json.paste")}
        </Button>
      </div>
      {!model && <p className="text-xs text-ink-soft">{t("pricing.json.copyBlocked")}</p>}
      <PanelOverlay open={open} onOpenChange={setOpen} desktop="dialog" title={t("pricing.json.pasteTitle")} description={t("pricing.json.pasteBody")}>
        {open && (
          <PasteForm
            onLoaded={(m) => {
              editor.replace(m);
              setOpen(false);
              toast.positive(t("pricing.json.loaded"));
            }}
          />
        )}
      </PanelOverlay>
    </div>
  );
}

function PasteForm({ onLoaded }: { onLoaded: (model: PricingModel) => void }) {
  const t = useT();
  const button = useButtonSize();
  const id = useId();
  const [text, setText] = useState("");
  const [refused, setRefused] = useState<{ notJson: true } | { problems: PricingProblem[] } | null>(null);
  const load = () => {
    const result = importModel(text);
    if (result.kind === "ok") return onLoaded(result.model);
    setRefused(result.kind === "not_json" ? { notJson: true } : { problems: result.problems });
  };
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        load();
      }}
    >
      <Field className="flex flex-col gap-1">
        <FieldLabel htmlFor={id}>{t("pricing.json.field")}</FieldLabel>
        <Textarea id={id} className={PASTE_FIELD_CLASS} aria-describedby={`${id}-hint`} spellCheck={false} value={text} aria-invalid={refused ? true : undefined} onChange={(e) => setText(e.target.value)} />
        <FieldDescription id={`${id}-hint`}>{t("pricing.json.hint")}</FieldDescription>
      </Field>
      {refused && (
        <Alert variant="destructive">
          <AlertDescription>
            {"notJson" in refused ? (
              t("pricing.json.notJson")
            ) : (
              <ul className={PROBLEMS_CLASS}>
                {refused.problems.slice(0, SHOWN_PROBLEMS).map((p, i) => (
                  <li key={i} className="wrap-anywhere">
                    <span className="font-mono">{p.path || "model"}</span>: {shownText({ code: p.code, vars: p.vars }, t)}
                  </li>
                ))}
                {refused.problems.length > SHOWN_PROBLEMS && <li>{t("pricing.json.more", { n: refused.problems.length - SHOWN_PROBLEMS })}</li>}
              </ul>
            )}
          </AlertDescription>
        </Alert>
      )}
      <Button type="submit" size={button()} className="self-start" disabled={text.trim() === ""}>
        {t("pricing.json.load")}
      </Button>
    </form>
  );
}
