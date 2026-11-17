import type { Stage } from "@/entities/lead";
import { lostReasonValues } from "@/shared/api/schema";
import type { T } from "@/shared/i18n";

export type MoveKind = "contacted" | "quoted" | "won" | "lost" | "completed";

/**
 * The next steps offered from a stage. Stages only move forward; `lost` is
 * reachable from anywhere before payment, and a lost lead can be reopened by
 * progress (`panel_core::lead::fold`). A paid lead has nowhere left to go.
 */
export const MOVES: Record<Stage, readonly MoveKind[]> = {
  created: ["contacted", "quoted", "lost"],
  contacted: ["quoted", "won", "lost"],
  quoted: ["won", "lost"],
  won: ["completed", "lost"],
  completed: [],
  paid: [],
  lost: ["contacted", "quoted", "won"],
};

/** Whether a lead in this stage has a next step at all; the card hides "Next step" otherwise. */
export function hasMoves(stage: Stage): boolean {
  return MOVES[stage].length > 0;
}

/** A reason as a person reads it; a slug of a `lead.lost` from before the panel's words shows as it is. */
export function lostReasonLabel(reason: string, t: T): string {
  const known = lostReasonValues.find((r) => r === reason);
  return known ? t(`lost.${known}`) : reason;
}
