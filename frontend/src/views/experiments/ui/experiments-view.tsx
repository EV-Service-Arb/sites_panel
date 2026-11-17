import { useResource } from "@evinvest/api/react";
import { Settled, Skeleton } from "@evinvest/uikit";

import { brandsOf, usePlaces, useSelectedBrands } from "@/entities/place";
import { useMe } from "@/entities/session";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { openView, useViewParams } from "@/shared/lib/router";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { FilterSelect } from "@/shared/ui/filter-select";
import { ScreenFrame } from "@/shared/ui/screen-frame";

import { byBrand } from "../model/group";
import { BrandExperiments } from "./brand-experiments";

/**
 * The landings' A/B tests as config: what each brand declared, the split it
 * runs, and for an admin the kill switch and the weights. The numbers are
 * PostHog's, a link away; the panel does not count them again. Those of the
 * brands the host's selection has a place of.
 */
export function ExperimentsView() {
  const t = useT();
  const me = useMe();
  const brand = useViewParams("experiments").brand ?? null;
  const update = ({ brand }: { brand: string | null }) => openView({ view: "experiments", params: brand === null ? {} : { brand } }, { replace: true });
  const places = usePlaces();
  const all = useResource(api(), "/experiments", { params: { query: { brand } } });
  const selection = useSelectedBrands();
  const failure = all.status === "error" ? all.failure : selection.status === "error" ? selection.failure : undefined;
  const selected = selection.status === "ok" ? selection.data : undefined;
  const ready = all.status === "ok" && selected !== undefined;
  const list = !ready ? [] : selected === null ? all.data.experiments : all.data.experiments.filter((e) => selected.has(e.brand));
  const brands = brandsOf([...places, ...list], brand);

  return (
    <ScreenFrame
      title={t("experiments.title")}
      actions={<FilterSelect label={t("filter.brand")} allLabel={t("filter.brand.all")} value={brand} options={brands.map((b) => ({ value: b, label: b }))} onChange={(b) => update({ brand: b })} />}
    >
      <Settled loading={!ready && failure === undefined} skeleton={<Skeleton className="h-64 w-full" />} className="flex flex-col gap-4">
        {failure !== undefined && <ErrorState failure={failure} onRetry={all.reload} />}
        {ready && list.length === 0 && <EmptyState title={t("experiments.empty")} description={t("experiments.empty.body")} />}
        {ready && list.length > 0 && (
          <>
            <p className="px-1 text-sm text-ink-soft">{t("experiments.how")}</p>
            {byBrand(list).map(([b, experiments]) => (
              <BrandExperiments key={b} brand={b} experiments={experiments} editable={api().mayCall("put", "/experiments/{brand}/{key}", me.permissions)} onSaved={all.reload} />
            ))}
          </>
        )}
      </Settled>
    </ScreenFrame>
  );
}
