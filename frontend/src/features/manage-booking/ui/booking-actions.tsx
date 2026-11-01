import { Button, type ButtonVariant, Dialog, DialogContent, DialogHeader, DialogTitle } from "@evinvest/uikit";
import { useState } from "react";

import { type BookingAction, type Lead, bookingActions, refOf } from "@/entities/lead";
import { type MessageKey, useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { useButtonSize } from "@/shared/ui/touch";

import { useBookingWrite } from "../model/use-booking-write";
import { SlotForm } from "./slot-form";

const VARIANT: Record<BookingAction, ButtonVariant> = { set: "outline", done: "outline", no_show: "outline", canceled: "ghost", clear: "ghost" };

/**
 * What an operator may do with the lead's booking from where it stands; the
 * rest would answer 409 and is not offered. Setting a time opens a dialog on
 * desktop; on a phone the card is already a drawer, so the form opens in place
 * rather than stacking a second overlay on it.
 */
export function BookingActions({ lead, onChanged }: { lead: Lead; onChanged: () => void }) {
  const t = useT();
  const button = useButtonSize();
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  const [editing, setEditing] = useState(false);
  const { busy, run } = useBookingWrite(refOf(lead), onChanged);
  const actions = bookingActions(lead.booking.status);
  const label = (a: BookingAction): MessageKey => (a === "set" && lead.booking.start_at !== null ? "booking.action.reschedule" : `booking.action.${a}`);

  const tap = (a: BookingAction) => {
    if (a === "set") setEditing(true);
    else void run(a, a === "clear" ? { action: "clear" } : { status: a });
  };
  const form = (
    <SlotForm
      booking={lead.booking}
      busy={busy}
      onCancel={() => setEditing(false)}
      onSubmit={(body) => void run("set", body).then((ok) => ok && setEditing(false))}
    />
  );

  if (editing && !isDesktop) return form;
  return (
    <>
      <div className="flex flex-wrap gap-2">
        {actions.map((a) => (
          <Button key={a} size={button()} variant={VARIANT[a]} disabled={busy} onClick={() => tap(a)}>
            {t(label(a))}
          </Button>
        ))}
      </div>
      {isDesktop && (
        <Dialog open={editing} onOpenChange={setEditing}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{t("slot.title")}</DialogTitle>
            </DialogHeader>
            {form}
          </DialogContent>
        </Dialog>
      )}
    </>
  );
}
