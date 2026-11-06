import { BOOKING_STATUSES, LEAD_CHANNELS, LEAD_FLOWS, messageRefOf } from "@evinvest/sites-contract";

import { type LeadCounts, type LeadFilter, STAGES, type Stage } from "@/entities/lead";
import type { MessageKey, T } from "@/shared/i18n";

const DAY = /^\d{4}-\d{2}-\d{2}$/;
const day = (v: string | null) => (v && DAY.test(v) ? v : null);

/** The filter lives in the URL, so a view can be linked and survives a reload. */
export function leadFilterFrom(params: URLSearchParams): LeadFilter {
  const stage = params.get("stage");
  return {
    stage: STAGES.find((s) => s === stage) ?? null,
    brand: params.get("brand") || null,
    location: params.get("location") || null,
    overdue: params.get("overdue") === "1",
    createdFrom: day(params.get("created_from")),
    createdTo: day(params.get("created_to")),
    suspect: suspectFilterOf(params.get("suspect")),
    flow: flowFilterOf(params.get("flow")),
    booking: bookingFilterOf(params.get("booking")),
    channel: channelFilterOf(params.get("channel")),
    messageRef: messageRefOf(params.get("message_ref")),
  };
}

export function paramsWith(params: URLSearchParams, patch: Partial<LeadFilter>): URLSearchParams {
  const next = new URLSearchParams(params);
  const set = (k: string, v: string | null) => (v ? next.set(k, v) : next.delete(k));
  if ("stage" in patch) set("stage", patch.stage ?? null);
  if ("brand" in patch) set("brand", patch.brand ?? null);
  if ("location" in patch) set("location", patch.location ?? null);
  if ("overdue" in patch) set("overdue", patch.overdue ? "1" : null);
  if ("createdFrom" in patch) set("created_from", patch.createdFrom ?? null);
  if ("createdTo" in patch) set("created_to", patch.createdTo ?? null);
  if ("suspect" in patch) set("suspect", patch.suspect ?? null);
  if ("flow" in patch) set("flow", patch.flow ?? null);
  if ("booking" in patch) set("booking", patch.booking ?? null);
  if ("channel" in patch) set("channel", patch.channel ?? null);
  if ("messageRef" in patch) set("message_ref", patch.messageRef ?? null);
  return next;
}

/** The URL's word, if it is one the API takes (anything else answers 400): "every lead" otherwise. */
export function suspectFilterOf(v: string | null): LeadFilter["suspect"] {
  return v === "only" || v === "exclude" ? v : null;
}

/** Whether any of `keys` narrows the list (every key of the filter by default). */
export function narrows(filter: LeadFilter, keys: readonly (keyof LeadFilter)[] = FILTER_KEYS): boolean {
  return keys.some((k) => filter[k] !== null && filter[k] !== false);
}

const FILTER_KEYS: readonly (keyof LeadFilter)[] = ["stage", "brand", "location", "overdue", "createdFrom", "createdTo", "suspect", "flow", "booking", "channel", "messageRef"];

/** As with suspect: a word the API does not take would answer 400, so it reads as "every lead". */
export function flowFilterOf(v: string | null): LeadFilter["flow"] {
  return LEAD_FLOWS.find((f) => f === v) ?? null;
}

/** A booking status the API takes (anything else answers 400): every lead otherwise. */
export function bookingFilterOf(v: string | null): LeadFilter["booking"] {
  return BOOKING_STATUSES.find((s) => s === v) ?? null;
}

/** A channel the API takes (anything else answers 400): every lead otherwise. */
export function channelFilterOf(v: string | null): LeadFilter["channel"] {
  return LEAD_CHANNELS.find((c) => c === v) ?? null;
}

export function stageOrNull(v: string | null): Stage | null {
  return STAGES.find((s) => s === v) ?? null;
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
