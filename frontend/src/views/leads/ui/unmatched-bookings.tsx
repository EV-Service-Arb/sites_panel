import { Badge, Button, Card, Collapsible, CollapsibleContent, CollapsibleTrigger, Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@evinvest/uikit";
import { ChevronDown } from "lucide-react";
import { useState } from "react";

import { type UnmatchedBooking, fetchUnmatched } from "@/entities/booking";
import { AttachDialog } from "@/features/attach-booking";
import { useLocale, useT } from "@/shared/i18n";
import { formatSlot } from "@/shared/lib/instant";
import { useResource } from "@/shared/lib/use-resource";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * Bookings a provider took that no lead was found for, under the queue's
 * brand. Folded, and absent while there are none: it is a chore that comes
 * up, not a screen. Follows the `bookings` topic live.
 */
export function UnmatchedBookings({ brand, version, onAttached }: { brand: string | null; version: number; onAttached: () => void }) {
  const t = useT();
  const [picked, setPicked] = useState<UnmatchedBooking | null>(null);
  const list = useResource(`unmatched:${brand ?? ""}:${version}`, () => fetchUnmatched(brand), `unmatched:${brand ?? ""}`, { live: ["bookings"] });
  const bookings = list.status === "ok" ? list.data : [];
  if (bookings.length === 0 && picked === null) return null;

  const attached = () => {
    setPicked(null);
    list.reload();
    onAttached();
  };
  return (
    <Card className="gap-0 overflow-hidden py-0">
      <Collapsible>
        <CollapsibleTrigger className="group flex w-full items-center gap-2 px-4 py-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring">
          <span className="font-medium text-ink">{t("unmatched.title")}</span>
          <Badge variant="primary">{bookings.length}</Badge>
          <ChevronDown aria-hidden className="ml-auto size-4 text-ink-soft transition-transform group-aria-expanded:rotate-180" />
        </CollapsibleTrigger>
        <CollapsibleContent className="border-t border-border">
          <p className="px-4 pt-3 text-sm text-ink-soft">{t("unmatched.description")}</p>
          <ItemGroup className="divide-y divide-border">
            {bookings.map((b) => (
              <UnmatchedRow key={b.id} booking={b} onAttach={() => setPicked(b)} />
            ))}
          </ItemGroup>
        </CollapsibleContent>
      </Collapsible>
      <AttachDialog booking={picked} onClose={() => setPicked(null)} onAttached={attached} />
    </Card>
  );
}

function UnmatchedRow({ booking, onAttach }: { booking: UnmatchedBooking; onAttach: () => void }) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  const c = booking.contact;
  const contact = [c?.name, c?.phone, c?.email].filter(Boolean).join(" · ");
  return (
    <Item size="sm" className="rounded-none max-md:flex-wrap">
      <ItemContent className="min-w-0">
        <ItemTitle className="flex flex-wrap items-center gap-2 tabular-nums">
          {formatSlot(booking.start_at, booking.end_at, locale)}
          {booking.status === "canceled" && <Badge variant="outline">{t("booking.status.canceled")}</Badge>}
        </ItemTitle>
        <ItemDescription>
          {booking.brand} · {t(`booking.provider.${booking.provider}`)}
        </ItemDescription>
        <ItemDescription className={contact ? "wrap-anywhere text-ink-mid" : "italic"}>{contact || t("unmatched.noContact")}</ItemDescription>
      </ItemContent>
      <ItemActions className="max-md:w-full">
        <Button variant="outline" size={button("sm")} className="max-md:w-full" onClick={onAttach}>
          {t("unmatched.attach")}
        </Button>
      </ItemActions>
    </Item>
  );
}
