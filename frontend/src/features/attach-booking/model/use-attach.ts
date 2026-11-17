import { toast } from "@evinvest/uikit";
import { useState } from "react";

import type { UnmatchedBooking } from "@/entities/booking";
import { api } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";

/**
 * Attaching, told in the operator's words: no such lead is one the brand does not
 * have (the dialog stays open to fix the id); attached already is someone else
 * first, and the list is read again.
 */
export function useAttach(booking: UnmatchedBooking, onDone: () => void) {
  const t = useT();
  const [busy, setBusy] = useState(false);

  const attach = async (lead: string) => {
    setBusy(true);
    const { error } = await api().POST("/bookings/{id}/attach", { params: { path: { id: booking.id } }, body: { lead } });
    setBusy(false);
    if (error === undefined) {
      toast.positive(t("attach.saved"));
      return onDone();
    }
    if (error.code === "not_found") return toast.error(t("attach.notFound", { brand: booking.brand, id: lead }));
    if (error.code === "already_attached") {
      toast.error(t("attach.conflict"));
      return onDone();
    }
    notifyFailure(error, t);
  };

  return { busy, attach };
}
