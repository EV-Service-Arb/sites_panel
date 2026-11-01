import { toast } from "@evinvest/uikit";

import { type BookingAction, type CloseBody, type LeadRef, type SlotBody, closeBooking, writeSlot } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { useKeyedWrite } from "@/shared/lib/use-keyed-write";
import { notifyFailure } from "@/shared/ui/notify";

import { bookingRefusal } from "./failure";

/** A booking write, told: saved, or why not; a 409 has the card read again. Resolves true once written. */
export function useBookingWrite(ref: LeadRef, onChanged: () => void) {
  const t = useT();
  const { busy, run: write, forget } = useKeyedWrite();

  const run = async (action: BookingAction, body: SlotBody | CloseBody): Promise<boolean> => {
    const outcome = await write({ action, body }, (key) => ("status" in body ? closeBooking(ref, body, key) : writeSlot(ref, body, key)));
    if (outcome.ok) {
      toast.positive(t(`booking.saved.${action}`));
      onChanged();
      return true;
    }
    const refusal = bookingRefusal(outcome.error);
    if (refusal === null) {
      notifyFailure(outcome.error, t);
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
