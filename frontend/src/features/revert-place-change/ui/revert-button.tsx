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

import type { PlaceKey, PlaceSettingsView } from "@/entities/place";
import { api } from "@/shared/api";
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
    const { data, error } = await api().POST("/places/{brand}/{slug}/settings/revert/{id}", {
      params: { path: { brand: place.brand, slug: place.slug, id: changeId } },
      body: { expected_updated_at: expectedUpdatedAt },
    });
    if (error?.code === "stale") return onConflict();
    if (error !== undefined) return notifyFailure(error, t);
    toast.positive(t("placeSettings.history.reverted"));
    onReverted(data);
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
