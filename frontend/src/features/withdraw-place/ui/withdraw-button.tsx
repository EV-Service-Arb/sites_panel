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

import { type PlaceSettingsView, setPlaceWithdrawn } from "@/entities/place";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * Admin only. Withdrawing makes the site answer 404 for the point, so the
 * confirmation says that in as many words; restoring is the same switch back.
 */
export function WithdrawButton({ place, onChanged }: { place: PlaceSettingsView; onChanged: (view: PlaceSettingsView) => void }) {
  const t = useT();
  const button = useButtonSize();
  const action = place.withdrawn ? "restore" : "withdraw";
  const run = async () => {
    try {
      const view = await setPlaceWithdrawn(place, !place.withdrawn);
      toast.positive(t(`placeSettings.${action}.done`));
      onChanged(view);
    } catch (e) {
      notifyFailure(e, t);
    }
  };
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant={place.withdrawn ? "outline" : "destructive"} size={button("sm")}>
          {t(`placeSettings.${action}`)}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t(`placeSettings.${action}.title`, { slug: place.slug })}</AlertDialogTitle>
          <AlertDialogDescription>{t(`placeSettings.${action}.body`)}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className={buttonVariants({ variant: "outline", size: button() })}>{t("move.cancel")}</AlertDialogCancel>
          <AlertDialogAction className={buttonVariants({ variant: place.withdrawn ? "primary" : "destructive", size: button() })} onClick={() => void run()}>
            {t(`placeSettings.${action}`)}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
