import { Badge, Button } from "@evinvest/uikit";
import { ArrowUpRight } from "lucide-react";

import { type Experiment, sharesOf, statusOf } from "@/entities/experiment";
import { ConfigureDialog, EnabledSwitch } from "@/features/configure-experiment";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime, formatPercent } from "@/shared/lib/format";

type Props = { experiment: Experiment };

export function KeyCell({ experiment }: Props) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <span className="break-all font-mono text-sm text-ink">{experiment.key}</span>
      {experiment.declared.summary && <span className="text-xs text-ink-soft">{experiment.declared.summary}</span>}
    </div>
  );
}

/** Running is the quiet state; off and holdout are what a glance down the list should catch. */
export function StatusBadge({ experiment }: Props) {
  const t = useT();
  const locale = useLocale();
  const status = statusOf(experiment);
  if (status === "running") return <Badge variant="success">{t("experiments.status.running")}</Badge>;
  if (status === "holdout") return <Badge variant="outline">{t("experiments.status.holdout", { share: formatPercent((experiment.effective.holdout ?? 0) * 100, locale) })}</Badge>;
  return <Badge variant="outline" className="text-ink-soft">{t(`experiments.status.${status}`)}</Badge>;
}

/** The effective split, the control first: "a 50 % · b 50 %". */
export function SplitList({ experiment }: Props) {
  const locale = useLocale();
  const shares = sharesOf(experiment.effective.weights);
  return (
    <ul className="flex flex-col gap-0.5 text-sm tabular-nums">
      {experiment.variants.map((v, i) => (
        <li key={v} className="flex justify-between gap-3">
          <span className="break-all font-mono text-ink-mid">{v}</span>
          <span className="text-ink">{formatPercent(shares[i] ?? 0, locale)}</span>
        </li>
      ))}
    </ul>
  );
}

/** When the code declared it, when the split last moved (PostHog's comparison starts there), and who changed it here. */
export function ChangeLines({ experiment }: Props) {
  const t = useT();
  const locale = useLocale();
  const { declared, override, weights_changed_at: weightsAt } = experiment;
  return (
    <ul className="flex flex-col gap-0.5 text-xs tabular-nums text-ink-soft">
      <li>{t("experiments.declaredAt", { at: formatDateTime(declared.declared_at, locale) })}</li>
      {weightsAt && <li>{t("experiments.weightsAt", { at: formatDateTime(weightsAt, locale) })}</li>}
      {override && <li>{t("experiments.changedBy", { by: override.changed_by, at: formatDateTime(override.changed_at, locale) })}</li>}
    </ul>
  );
}

export function PostHogLink({ experiment }: Props) {
  const t = useT();
  if (experiment.posthog_url === null) return null;
  return (
    <Button variant="link" size="sm" className="h-auto px-0" asChild>
      <a href={experiment.posthog_url} target="_blank" rel="noopener noreferrer" aria-label={t("experiments.posthog.label", { key: experiment.key })}>
        {t("experiments.posthog")}
        <ArrowUpRight aria-hidden className="size-4" />
      </a>
    </Button>
  );
}

/** An admin's controls; a retired experiment has none — no landing reads its config any more. */
export function Controls({ experiment, onSaved }: Props & { onSaved: () => void }) {
  if (experiment.retired) return null;
  return (
    <div className="flex items-center gap-3">
      <EnabledSwitch experiment={experiment} onSaved={onSaved} />
      <ConfigureDialog experiment={experiment} onSaved={onSaved} />
    </div>
  );
}
