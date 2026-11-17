import type { Funnel, FunnelStep, LocationFunnel } from "@/shared/api/schema";

/** `GET /funnel`'s answer without `by`: one slice. */
export type WholeFunnel = Extract<Funnel, { stages: unknown }>;

export function wholeOf(funnel: Funnel): WholeFunnel {
  if (!("stages" in funnel)) throw new Error("/funnel answered by location where the whole funnel was asked");
  return funnel;
}

/** `GET /funnel?by=location`'s rows. */
export function byLocation(funnel: Funnel): LocationFunnel[] {
  if (!("locations" in funnel)) throw new Error("/funnel?by=location answered the whole funnel");
  return funnel.locations;
}

/**
 * The step that loses the most leads, counted in leads (a percent may be
 * withheld for a small sample, a count never is). None when nothing is lost,
 * and none on a tie — two "biggest" losses would point nowhere.
 */
export function biggestLoss(steps: readonly FunnelStep[]): FunnelStep["stage"] | null {
  let best: { stage: FunnelStep["stage"]; lost: number } | null = null;
  let tie = false;
  for (const step of steps) {
    if (!step.of_previous) continue;
    const lost = step.of_previous.of - step.reached;
    if (lost <= 0) continue;
    if (!best || lost > best.lost) {
      best = { stage: step.stage, lost };
      tie = false;
    } else if (lost === best.lost) tie = true;
  }
  return best && !tie ? best.stage : null;
}
