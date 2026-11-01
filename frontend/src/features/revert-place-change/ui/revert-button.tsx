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

import { type PlaceKey, type PlaceSettingsView, revertSettingsChange } from "@/entities/place";
import { ApiError } from "@/shared/api";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { useButtonSize } from "@/shared/ui/touch";

export interface RevertButtonProps {
  place: PlaceKey;
  changeId: string;
  /** The place's `updated_at` as last read; a newer one on the server makes this a 409. */
  expectedUpdatedAt: string | null;
  onReverted: (view: PlaceSettingsView) => void;
  onConflict: () => void;
}

/** Puts a change's `before` back, after a confirmation that says the revert is itself recorded. */
export function RevertButton({ place, changeId, expectedUpdatedAt, onReverted, onConflict }: RevertButtonProps) {
  const t = useT();
  const button = useButtonSize();
  const revert = async () => {
    try {
      const view = await revertSettingsChange(place, changeId, expectedUpdatedAt);
      toast.positive(t("placeSettings.history.reverted"));
      onReverted(view);
    } catch (e) {
      if (e instanceof ApiError && e.failure.kind === "conflict") return onConflict();
      notifyFailure(e, t);
    }
  };
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="outline" size={button("sm")}>
          {t("placeSettings.history.revert")}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("placeSettings.history.revert.title")}</AlertDialogTitle>
          <AlertDialogDescription>{t("placeSettings.history.revert.body")}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className={buttonVariants({ variant: "outline", size: button() })}>{t("move.cancel")}</AlertDialogCancel>
          <AlertDialogAction className={buttonVariants({ variant: "primary", size: button() })} onClick={() => void revert()}>
            {t("placeSettings.history.revert")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
