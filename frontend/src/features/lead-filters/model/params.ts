import { messageRefOf } from "@evinvest/sites-contract";

import type { LeadCounts, LeadFilter, Stage } from "@/entities/lead";
import { bookingStatusValues, leadChannelValues, leadFlowValues, stageValues, suspectFilterValues } from "@/shared/api/schema";
import type { MessageKey, T } from "@/shared/i18n";

const DAY = /^\d{4}-\d{2}-\d{2}$/;
const day = (v: string | null) => (v && DAY.test(v) ? v : null);
const wordOf = <W extends string>(words: readonly W[], v: string | null): W | null => words.find((w) => w === v) ?? null;

/** The filter lives in the URL, so a view can be linked and survives a reload; the places are the host's selection, not here. */
export function leadFilterFrom(params: URLSearchParams): LeadFilter {
  return {
    stage: wordOf(stageValues, params.get("stage")),
    brand: params.get("brand") || null,
    overdue: params.get("overdue") === "1",
    created_from: day(params.get("created_from")),
    created_to: day(params.get("created_to")),
    suspect: wordOf(suspectFilterValues, params.get("suspect")),
    flow: wordOf(leadFlowValues, params.get("flow")),
    booking: wordOf(bookingStatusValues, params.get("booking")),
    channel: wordOf(leadChannelValues, params.get("channel")),
    message_ref: messageRefOf(params.get("message_ref")),
  };
}

export function paramsWith(params: URLSearchParams, patch: Partial<LeadFilter>): URLSearchParams {
  const next = new URLSearchParams(params);
  for (const [k, v] of Object.entries(patch)) {
    const text = k === "overdue" ? (v ? "1" : null) : typeof v === "string" && v !== "" ? v : null;
    if (text === null) next.delete(k);
    else next.set(k, text);
  }
  return next;
}

const FILTER_KEYS: readonly (keyof LeadFilter)[] = ["stage", "brand", "overdue", "created_from", "created_to", "suspect", "flow", "booking", "channel", "message_ref"];

/** Whether any of `keys` narrows the list (every key of the filter by default). */
export function narrows(filter: LeadFilter, keys: readonly (keyof LeadFilter)[] = FILTER_KEYS): boolean {
  return keys.some((k) => filter[k] !== undefined && filter[k] !== null && filter[k] !== false);
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
