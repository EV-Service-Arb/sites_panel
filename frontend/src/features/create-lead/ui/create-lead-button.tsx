import { Button, Field, FieldLabel, Input, Textarea, toast } from "@evinvest/uikit";
import { useId, useState } from "react";

import { type LeadRef, type ManualChannel, createLead } from "@/entities/lead";
import { type Place, locationsOf } from "@/entities/place";
import { isSlug } from "@/shared/config/brands";
import { useT } from "@/shared/i18n";
import { notifyFailure } from "@/shared/ui/notify";
import { PanelOverlay } from "@/shared/ui/panel-overlay";
import { useButtonSize, useControlSize } from "@/shared/ui/touch";

import { readLastPlace, writeLastPlace } from "../model/last-place";
import { ChannelField } from "./channel-field";
import { PlaceFields, type PlaceValue } from "./place-fields";

const EMPTY: PlaceValue = { brand: "", location: null, custom: "" };

/**
 * "+ Call": a call — or a WhatsApp / Telegram message — that bypassed the form
 * becomes a lead: location (the last one by default), how they reached us,
 * what they need, and the phone if given.
 */
export function CreateLeadButton({ brands, places, onCreated }: { brands: string[]; places: readonly Pick<Place, "brand" | "location">[]; onCreated: (ref: LeadRef) => void }) {
  const t = useT();
  const button = useButtonSize();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [place, setPlace] = useState<PlaceValue>(EMPTY);
  const [need, setNeed] = useState("");
  const [phone, setPhone] = useState("");
  const [channel, setChannel] = useState<ManualChannel>("phone_inbound");
  const [busy, setBusy] = useState(false);
  const size = useControlSize();

  const locations = place.brand ? locationsOf(places, place.brand) : [];
  const location = place.location ?? place.custom.trim();
  const ready = place.brand !== "" && isSlug(location) && need.trim() !== "";

  const openForm = () => {
    const last = readLastPlace();
    setPlace(last ? { brand: last.brand, location: last.location, custom: "" } : EMPTY);
    setNeed("");
    setPhone("");
    setChannel("phone_inbound");
    setOpen(true);
  };

  const submit = async () => {
    setBusy(true);
    try {
      const created = await createLead({ brand: place.brand, location, need: need.trim(), phone: phone.trim() || null, channel });
      writeLastPlace({ brand: place.brand, location });
      toast.positive(t("create.saved"));
      setOpen(false);
      onCreated({ brand: created.brand, lead: created.lead_id });
    } catch (e) {
      notifyFailure(e, t);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Button size={button()} onClick={openForm}>{t("leads.newCall")}</Button>
      <PanelOverlay open={open} onOpenChange={setOpen} title={t("create.title")} description={t("create.description")} desktop="dialog">
        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (ready) void submit();
          }}
        >
          <PlaceFields brands={brands} locations={locations} value={place} onChange={setPlace} />
          <ChannelField value={channel} onChange={setChannel} />
          <Field className="flex flex-col gap-1">
            <FieldLabel htmlFor={`${id}-need`}>{t("create.need")}</FieldLabel>
            <Textarea id={`${id}-need`} size={size} rows={2} maxLength={1000} value={need} onChange={(e) => setNeed(e.target.value)} />
          </Field>
          <Field className="flex flex-col gap-1">
            <FieldLabel htmlFor={`${id}-phone`}>{t("create.phone")}</FieldLabel>
            <Input id={`${id}-phone`} size={size} type="tel" inputMode="tel" autoComplete="off" value={phone} onChange={(e) => setPhone(e.target.value)} />
          </Field>
          <Button type="submit" size={button("lg")} disabled={busy || !ready}>
            {t("create.submit")}
          </Button>
        </form>
      </PanelOverlay>
    </>
  );
}
