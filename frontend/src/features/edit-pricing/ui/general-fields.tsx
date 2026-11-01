import { useT } from "@/shared/i18n";

import { FIELD } from "../model/fields";
import type { PricingEditor } from "../model/use-pricing-editor";
import { DayField } from "./day-field";
import { TextField } from "./text-field";

/** What holds for the whole model: from when, the estimate's rounding step and its floor. */
export function GeneralFields({ editor }: { editor: PricingEditor }) {
  const t = useT();
  const { draft, errors } = editor;
  const set = (patch: Partial<Pick<typeof draft, "validFrom" | "roundTo" | "minimum">>) => editor.update((d) => ({ ...d, ...patch }));
  return (
    <section className="grid items-start gap-3 sm:grid-cols-3" aria-label={t("pricing.general.title")}>
      <DayField field={FIELD.validFrom} label={t("pricing.field.validFrom")} value={draft.validFrom} onChange={(validFrom) => set({ validFrom })} errors={errors} />
      <TextField field={FIELD.roundTo} label={t("pricing.field.roundTo")} hint={t("pricing.hint.roundTo")} kind="amount" value={draft.roundTo} onChange={(roundTo) => set({ roundTo })} errors={errors} />
      <TextField field={FIELD.minimum} label={t("pricing.field.minimum")} hint={t("pricing.hint.minimum")} kind="amount" value={draft.minimum} onChange={(minimum) => set({ minimum })} errors={errors} />
    </section>
  );
}
