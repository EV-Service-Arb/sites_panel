import { type LostReason, MOVES, type Stage } from "@/entities/lead";
import type { T } from "@/shared/i18n";

export type MoveKind = (typeof MOVES)[Stage][number];

/** Whether a lead in this stage has a next step at all; the card hides "Next step" otherwise. */
export function hasMoves(stage: Stage): boolean {
  return MOVES[stage].length > 0;
}

export function lostReasonLabel(reason: LostReason, t: T): string {
  return t(`lost.${reason}`);
}
