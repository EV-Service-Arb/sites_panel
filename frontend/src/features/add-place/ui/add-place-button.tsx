import { Button, Field, FieldDescription, FieldError, FieldLabel, Input, Select, SelectContent, SelectItem, SelectTrigger, SelectValue, toast } from "@evinvest/uikit";
import { Plus } from "lucide-react";
import { useId, useState } from "react";

import { type PlaceKey, addPlace } from "@/entities/place";
import { ApiError } from "@/shared/api";
import { isSlug } from "@/shared/config/brands";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { useButtonSize, useControlSize } from "@/shared/ui/touch";

/**
 * Admin only: registers a point no lead or visit has named yet, so its site
 * data can be set before the first lead. The brand is one the panel knows: a place's,
 * or one an active source writes for.
 */
export function AddPlaceButton({ brands, onAdded }: { brands: string[]; onAdded: (key: PlaceKey) => void }) {
  const t = useT();
  const button = useButtonSize();
  const size = useControlSize();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [brand, setBrand] = useState("");
  const [slug, setSlug] = useState("");
  const [busy, setBusy] = useState(false);
  // The pair the backend said exists; editing either field clears it.
  const [exists, setExists] = useState<PlaceKey | null>(null);
  const ready = brands.includes(brand) && isSlug(slug);

  const submit = async () => {
    setBusy(true);
    try {
      await addPlace({ brand, slug });
      toast.positive(t("addPlace.done", { brand, slug }));
      setOpen(false);
      setSlug("");
      onAdded({ brand, slug });
    } catch (e) {
      if (e instanceof ApiError && e.failure.kind === "conflict") return setExists({ brand, slug });
      notifyFailure(e, t);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Button variant="outline" size={button("sm")} onClick={() => setOpen(true)} disabled={brands.length === 0}>
        <Plus aria-hidden="true" />
        {t("addPlace.button")}
      </Button>
      <PanelOverlay open={open} onOpenChange={setOpen} desktop="dialog" title={t("addPlace.title")} description={t("addPlace.description")}>
        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (ready) void submit();
          }}
        >
          <Field className="flex flex-col gap-1">
            <FieldLabel htmlFor={`${id}-brand`}>{t("addPlace.brand")}</FieldLabel>
            <Select
              value={brand}
              onValueChange={(b) => {
                setBrand(b);
                setExists(null);
              }}
            >
              <SelectTrigger id={`${id}-brand`} size={size} className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {brands.map((b) => (
                  <SelectItem key={b} value={b}>
                    {b}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          <Field className="flex flex-col gap-1">
            <FieldLabel htmlFor={`${id}-slug`}>{t("addPlace.slug")}</FieldLabel>
            <Input id={`${id}-slug`} size={size} autoCapitalize="none" value={slug} onChange={(e) => {
                setSlug(e.target.value.toLowerCase().trim());
                setExists(null);
              }}
            />
            <FieldDescription>{t("addPlace.slug.hint")}</FieldDescription>
            {exists && <FieldError>{t("addPlace.exists", { brand: exists.brand, slug: exists.slug })}</FieldError>}
          </Field>
          <Button type="submit" size={button()} disabled={busy || !ready || exists !== null} className="self-end">
            {t("addPlace.submit")}
          </Button>
        </form>
      </PanelOverlay>
    </>
  );
}
