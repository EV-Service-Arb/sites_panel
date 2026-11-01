import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
  Button,
  buttonVariants,
  toast,
} from "@evinvest/uikit";

import type { PricingItem } from "@/entities/pricing";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

import { tryClear } from "../model/clear";

/**
 * Admin only. Taking the model off sends every site of the brand back to the
 * prices built into it, so the confirmation says that in as many words.
 */
export function ClearPricingButton({ item, onCleared }: { item: PricingItem; onCleared: (item?: PricingItem) => void }) {
  const t = useT();
  const button = useButtonSize();
  const run = async () => {
    const outcome = await tryClear(item);
    switch (outcome.kind) {
      case "cleared":
        toast.positive(t("pricing.clear.done"));
        return onCleared(outcome.item);
      case "conflict":
        // Someone saved first: what is current is shown rather than removed unseen.
        toast.error(t("pricing.clear.conflict"));
        return onCleared(outcome.current ?? undefined);
      case "failed":
        notifyFailure(outcome.error, t);
        return onCleared();
    }
  };
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size={button("sm")} disabled={item.model === null}>
          {t("pricing.clear")}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("pricing.clear.title", { brand: item.brand_id })}</AlertDialogTitle>
          <AlertDialogDescription>{t("pricing.clear.body")}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className={buttonVariants({ variant: "outline", size: button() })}>{t("move.cancel")}</AlertDialogCancel>
          <AlertDialogAction className={buttonVariants({ variant: "destructive", size: button() })} onClick={() => void run()}>
            {t("pricing.clear")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
