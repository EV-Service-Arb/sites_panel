import { daysAgo, utcDay } from "@/shared/lib/format";
import type { ViewParams } from "@/shared/lib/router";
import { VIEWS } from "@/shared/lib/views.generated";

export type Period = NonNullable<ViewParams<"overview">["period"]>;

/** The overview view's periods, as its `period` key takes them. */
export const PERIODS = VIEWS.views.overview.query.properties.period.enum.filter((p) => p !== null);

/** The last `days` UTC days, today included. */
export function rangeOf(days: number, now: Date): { from: string; to: string } {
  return { from: utcDay(daysAgo(now, days - 1)), to: utcDay(now) };
}
