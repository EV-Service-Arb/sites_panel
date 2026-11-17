import type { LeadCounts, LeadFilter, Stage } from "@/entities/lead";
import type { MessageKey, T } from "@/shared/i18n";
import type { ViewParams } from "@/shared/lib/router";

type Patch<T> = { [K in keyof T]?: T[K] | null };
/** A change to the filter: a key set to null narrows by it no more. */
export type FilterPatch = Patch<LeadFilter>;

/** The leads view's params with `patch` over them: a key set to nothing (null, false, an empty ref) leaves the URL. */
export function leadsWith(params: ViewParams<"leads">, patch: Patch<ViewParams<"leads">>): ViewParams<"leads"> {
  const merged = Object.entries({ ...params, ...patch }).filter(([, v]) => v !== null && v !== undefined && v !== false && v !== "");
  return Object.fromEntries(merged) as ViewParams<"leads">;
}

/** Whether any of `keys` narrows the list (every key of the filter by default). */
export function narrows(filter: LeadFilter, keys?: readonly (keyof LeadFilter)[]): boolean {
  return (keys ?? (Object.keys(filter) as (keyof LeadFilter)[])).some((k) => filter[k] !== undefined && filter[k] !== false);
}

/**
 * The phone's stage segments (the mockup's "New · In progress · Quotes"): the
 * three stages that wait on the operator. The API filters by one stage, so
 * "in progress" is `contacted`.
 */
export const SEGMENTS: readonly { stage: Stage; key: MessageKey }[] = [
  { stage: "created", key: "filter.segment.new" },
  { stage: "contacted", key: "filter.segment.inProgress" },
  { stage: "quoted", key: "filter.segment.quotes" },
];

/** "New · 3": a segment with how many leads wait in it; the bare name until the counts arrive. */
export function segmentLabels(counts: LeadCounts | null, t: T): { stage: Stage; label: string }[] {
  return SEGMENTS.map(({ stage, key }) => ({
    stage,
    label: counts ? t("filter.segment.count", { label: t(key), n: counts.stages[stage] }) : t(key),
  }));
}
