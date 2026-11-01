import { ToggleGroup, ToggleGroupItem } from "@evinvest/uikit";

import type { LeadCounts, Stage } from "@/entities/lead";
import { useT } from "@/shared/i18n";

import { SEGMENTS, segmentLabels } from "../model/params";

/**
 * "New · 3 | In progress · 1 | Quotes · 2": the stages an operator clears from a
 * phone, with how many wait in each. Tapping the lit one shows all.
 */
export function StageSegments({ stage, counts, onChange }: { stage: Stage | null; counts: LeadCounts | null; onChange: (stage: Stage | null) => void }) {
  const t = useT();
  const value = SEGMENTS.some((s) => s.stage === stage) ? (stage ?? "") : "";
  return (
    <ToggleGroup
      type="single"
      variant="outline"
      size="xl"
      aria-label={t("filter.stage")}
      className="w-full"
      value={value}
      onValueChange={(v) => onChange(typeof v === "string" ? (SEGMENTS.find((s) => s.stage === v)?.stage ?? null) : null)}
    >
      {segmentLabels(counts, t).map((s) => (
        <ToggleGroupItem key={s.stage} value={s.stage} className="min-w-0 flex-1 truncate tabular-nums">
          {s.label}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}
