import { useLocale, useT } from "@/shared/i18n";
import { formatCents } from "@/shared/lib/format";

import { effectText } from "../lib/amounts";
import { optionValues } from "../lib/effect";
import { labelOf } from "../lib/labels";
import { PRICING_CURRENCY, type PricingInput, type PricingModel } from "../model/model";

/** The model read, not edited: what an operator sees, and what a quote on the phone is checked against. */
export function ModelSummary({ model }: { model: PricingModel }) {
  const t = useT();
  const locale = useLocale();
  const money = (c: number) => formatCents(c, PRICING_CURRENCY, locale);
  const labelById = (id: string) => labelOf(model.inputs.find((i) => i.id === id)?.labels ?? {}, locale) || id;
  return (
    <div className="flex flex-col gap-4 text-sm">
      <p className="text-ink-mid">{t("pricing.summary.general", { round: money(model.roundToCents), minimum: money(model.minimumCents) })}</p>
      <section className="flex flex-col gap-2" aria-label={t("pricing.needs.title")}>
        <h3 className="font-medium text-ink">{t("pricing.needs.title")}</h3>
        <ul className="flex flex-col gap-1">
          {Object.entries(model.needs).map(([id, need]) => (
            <li key={id} className="wrap-anywhere text-ink">
              <span className="font-mono">{id}</span> ·{" "}
              {need.kind === "fixed" ? t("pricing.summary.fixed", { price: money(need.cents) }) : t("pricing.summary.estimate", { base: money(need.baseCents), inputs: need.inputs.map(labelById).join(", ") || "—" })}
            </li>
          ))}
        </ul>
      </section>
      <section className="flex flex-col gap-3" aria-label={t("pricing.inputs.title")}>
        <h3 className="font-medium text-ink">{t("pricing.inputs.title")}</h3>
        {model.inputs.map((input) => (
          <InputSummary key={input.id} input={input} />
        ))}
      </section>
    </div>
  );
}

function InputSummary({ input }: { input: PricingInput }) {
  const t = useT();
  const locale = useLocale();
  return (
    <div className="flex flex-col gap-1">
      <span className="text-ink">
        {labelOf(input.labels, locale)} <span className="font-mono text-ink-soft">({input.id})</span> · {t(`pricing.kind.${input.kind}`)}
      </span>
      <ul className="flex flex-col gap-0.5 ps-4 text-ink-mid">
        {optionValues(input).map((o) => (
          <li key={o.id} className="wrap-anywhere">
            {labelOf(o.labels, locale)} — {effectText(input.kind, o.value, locale)}
          </li>
        ))}
      </ul>
    </div>
  );
}
