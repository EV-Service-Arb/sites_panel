import { useResource } from "@evinvest/api/react";
import { Card, CardContent, CardHeader, CardTitle, Collapsible, CollapsibleContent, CollapsibleTrigger, Skeleton, buttonVariants, cn } from "@evinvest/uikit";
import { ChevronDown } from "lucide-react";
import { useState } from "react";

import type { PricingChange } from "@/entities/pricing";
import { api } from "@/shared/api";
import { useLocale, useT } from "@/shared/i18n";
import { formatDay, formatMoment } from "@/shared/lib/format";
import { useRereadOn } from "@/shared/lib/use-reread-on";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";

/** The brand's changes, newest first, folded away until asked for: read only while open. */
export function PricingHistory({ brand, version }: { brand: string; version: number }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  return (
    <Card>
      <Collapsible open={open} onOpenChange={setOpen}>
        <CardHeader>
          <CollapsibleTrigger className={cn(buttonVariants({ variant: "ghost" }), "w-full justify-between px-0")}>
            <CardTitle>{t("pricing.history.title")}</CardTitle>
            <ChevronDown aria-hidden className={cn("size-4 transition-transform", open && "rotate-180")} />
          </CollapsibleTrigger>
        </CardHeader>
        <CollapsibleContent>
          <CardContent>{open && <ChangeList brand={brand} version={version} />}</CardContent>
        </CollapsibleContent>
      </Collapsible>
    </Card>
  );
}

function ChangeList({ brand, version }: { brand: string; version: number }) {
  const t = useT();
  const changes = useResource(api(), "/pricing/{brand}/changes", { params: { path: { brand } } });
  useRereadOn(version, changes.reload);
  if (changes.status === "loading") return <Skeleton className="h-24 w-full" />;
  if (changes.status === "error") return <ErrorState failure={changes.failure} onRetry={changes.reload} />;
  if (changes.data.changes.length === 0) return <EmptyState className="p-4" title={t("pricing.history.empty")} />;
  return (
    <ul className="flex flex-col">
      {changes.data.changes.map((c) => (
        <ChangeRow key={c.id} change={c} />
      ))}
    </ul>
  );
}

function ChangeRow({ change }: { change: PricingChange }) {
  const t = useT();
  const locale = useLocale();
  const parts = change.changed;
  return (
    <li className="flex flex-col gap-0.5 border-b border-border py-2 text-sm last:border-b-0">
      <span className="text-ink-mid">{t("pricing.history.by", { at: formatMoment(change.at, locale), by: change.by })}</span>
      <span className="text-ink">
        {change.model
          ? t("pricing.history.set", { day: formatDay(change.model.validFrom, locale), inputs: change.model.inputs.length, needs: Object.keys(change.model.needs).length })
          : t("pricing.history.cleared")}
      </span>
      {parts && (
        <span className="text-ink-soft">
          {parts.length === 0 ? t("pricing.history.same") : t("pricing.history.changed", { parts: parts.map((p) => t(`pricing.history.part.${p}`)).join(", ") })}
        </span>
      )}
    </li>
  );
}
