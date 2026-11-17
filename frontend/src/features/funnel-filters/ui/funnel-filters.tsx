import { ToggleGroup, ToggleGroupItem } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { FilterSelect } from "@/shared/ui/filter-select";

import { PERIODS, type Period } from "../model/period";

/**
 * Period and brand: all the overview is cut by (spec §10.1 — nothing it cannot answer). No period
 * pressed is the funnel's own window.
 */
export function FunnelFilters({ period, brand, brands, onChange }: { period: Period | undefined; brand: string | undefined; brands: string[]; onChange: (patch: { period?: Period; brand?: string | null }) => void }) {
  const t = useT();
  const pick = (v: string) => {
    const next = PERIODS.find((p) => p === v);
    // Pressing the lit one again would clear it; the toggles pick a period, they do not take it away.
    if (next !== undefined) onChange({ period: next });
  };
  return (
    <div className="flex flex-wrap items-center gap-2">
      <ToggleGroup type="single" variant="outline" size="sm" aria-label={t("filter.period")} value={period ?? ""} onValueChange={(v) => typeof v === "string" && pick(v)}>
        {PERIODS.map((p) => (
          <ToggleGroupItem key={p} value={p} className="px-3">
            {t(`filter.period.${p}`)}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <FilterSelect label={t("filter.brand")} allLabel={t("filter.brand.all")} value={brand ?? null} options={brands.map((b) => ({ value: b, label: b }))} onChange={(b) => onChange({ brand: b })} />
    </div>
  );
}
