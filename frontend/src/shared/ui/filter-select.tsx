import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@evinvest/uikit";
import { useControlSize } from "./touch";

const ALL = "__all__";

export interface FilterOption<V extends string> {
  value: V;
  label: string;
}

/** A filter over one field: "all" or one of `options`. `null` is "all". */
export function FilterSelect<V extends string>({ label, allLabel, value, options, onChange }: { label: string; allLabel: string; value: V | null; options: FilterOption<V>[]; onChange: (v: V | null) => void }) {
  const size = useControlSize();
  const pick = (v: string) => {
    if (v === ALL) return onChange(null);
    const option = options.find((o) => o.value === v);
    if (option === undefined) throw new Error(`${label}: ${v} is none of its options`);
    onChange(option.value);
  };
  return (
    <Select value={value ?? ALL} onValueChange={pick}>
      <SelectTrigger size={size === "lg" ? "lg" : "sm"} aria-label={label} className="min-w-36 max-md:w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL}>{allLabel}</SelectItem>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
