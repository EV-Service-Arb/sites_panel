import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

/** Which brand's prices are on screen; the choice lives in the address (`?brand=`), so a link opens it. */
export function BrandSelect({ brands, value, onChange }: { brands: readonly string[]; value: string; onChange: (brand: string) => void }) {
  const t = useT();
  const size = useControlSize();
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger size={size === "lg" ? "lg" : "sm"} aria-label={t("pricing.brand")} className="min-w-40 max-md:w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {brands.map((b) => (
          <SelectItem key={b} value={b}>
            {b}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
