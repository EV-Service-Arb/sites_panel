import { toast } from "@evinvest/uikit";

import type { BookingAction, CloseBody, LeadRef, SlotBody } from "@/entities/lead";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { useKeyedWrite } from "@/shared/lib/use-keyed-write";
import { notifyFailure } from "@/shared/ui/notify";

import { bookingRefusal } from "./failure";

/** A booking write, told: saved, or why not; a conflict has the card read again. Resolves true once written. */
export function useBookingWrite(ref: LeadRef, onChanged: () => void) {
  const t = useT();
  const { busy, run: write, forget } = useKeyedWrite();

  const run = async (action: BookingAction, body: SlotBody | CloseBody): Promise<boolean> => {
    const outcome = await write({ action, body }, (key) => {
      const params = { path: ref, header: { "Idempotency-Key": key } };
      return "status" in body ? api().POST("/leads/{brand}/{lead}/booking/status", { params, body }) : api().POST("/leads/{brand}/{lead}/booking", { params, body });
    });
    if (outcome.ok) {
      toast.positive(t(`booking.saved.${action}`));
      onChanged();
      return true;
    }
    const refusal = bookingRefusal(outcome.failure);
    if (refusal === null) {
      notifyFailure(outcome.failure, t);
      return false;
    }
    toast.error(refusal.detail === undefined ? t(refusal.key) : t(refusal.key, { detail: refusal.detail }));
    if (refusal.reread) {
      forget();
      onChanged();
    }
    return false;
  };

  return { busy, run };
}
