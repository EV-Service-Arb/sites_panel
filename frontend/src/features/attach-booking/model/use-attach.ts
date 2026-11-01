import { toast } from "@evinvest/uikit";

import { type UnmatchedBooking, attachBooking } from "@/entities/booking";
import { ApiError } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { useKeyedWrite } from "@/shared/lib/use-keyed-write";
import { notifyFailure } from "@/shared/ui/notify";

/**
 * Attaching, told in the operator's words: a 404 is a lead the brand does not
 * have (the dialog stays open to fix the id); a 409 is someone attaching it
 * first, and the list is read again.
 */
export function useAttach(booking: UnmatchedBooking, onDone: () => void) {
  const t = useT();
  const { busy, run, forget } = useKeyedWrite();

  const attach = async (lead: string) => {
    const outcome = await run({ id: booking.id, lead }, (key) => attachBooking(booking.id, lead, key));
    if (outcome.ok) {
      toast.positive(t("attach.saved"));
      return onDone();
    }
    const kind = outcome.error instanceof ApiError ? outcome.error.failure.kind : null;
    if (kind === "not_found") toast.error(t("attach.notFound", { brand: booking.brand, id: lead }));
    else if (kind === "conflict") {
      forget();
      toast.error(t("attach.conflict"));
      onDone();
    } else notifyFailure(outcome.error, t);
  };

  return { busy, attach };
}
