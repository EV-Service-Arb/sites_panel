import { ToggleGroup, ToggleGroupItem } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { FilterSelect } from "@/shared/ui/filter-select";

import { PERIODS, type Period, periodFrom } from "../model/period";
import type { FilterPatch } from "../model/use-filter-params";

/** Period and brand: all the overview is cut by (spec §10.1 — nothing it cannot answer). */
export function FunnelFilters({ period, brand, brands, onChange }: { period: Period; brand: string | null; brands: string[]; onChange: (patch: FilterPatch) => void }) {
  const t = useT();
  return (
    <div className="flex flex-wrap items-center gap-2">
      <ToggleGroup
        type="single"
        variant="outline"
        size="sm"
        aria-label={t("filter.period")}
        value={String(period)}
        onValueChange={(v) => typeof v === "string" && v !== "" && onChange({ period: periodFrom(v) })}
      >
        {PERIODS.map((p) => (
          <ToggleGroupItem key={p} value={String(p)} className="px-3">
            {t(`filter.period.${p}`)}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <FilterSelect label={t("filter.brand")} allLabel={t("filter.brand.all")} value={brand} options={brands.map((b) => ({ value: b, label: b }))} onChange={(b) => onChange({ brand: b })} />
    </div>
  );
}
