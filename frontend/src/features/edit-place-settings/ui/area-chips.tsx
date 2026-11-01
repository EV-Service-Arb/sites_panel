import { Button, FieldError, Input } from "@evinvest/uikit";
import { X } from "lucide-react";
import { useState } from "react";

import { useT } from "@/shared/i18n";
import { useButtonSize, useControlSize } from "@/shared/ui/touch";

import { addArea, removeArea } from "../model/draft";

/** The communes the van goes to, as chips; Enter or "Add" puts the typed one in. */
export function AreaChips({ names, onChange, errors }: { names: string[]; onChange: (names: string[]) => void; errors: string[] | undefined }) {
  const t = useT();
  const button = useButtonSize();
  const size = useControlSize();
  const [typed, setTyped] = useState("");
  const add = () => {
    onChange(addArea(names, typed));
    setTyped("");
  };
  return (
    <section className="flex flex-col gap-2" aria-label={t("placeSettings.field.serviceArea")}>
      <h3 className="text-sm font-medium text-ink">{t("placeSettings.field.serviceArea")}</h3>
      {names.length === 0 ? (
        <p className="text-sm text-ink-soft">{t("placeSettings.area.empty")}</p>
      ) : (
        <ul className="flex flex-wrap gap-2">
          {names.map((name) => (
            <li key={name}>
              {/* The whole chip removes it: a separate small cross would be under the touch minimum. */}
              <Button type="button" variant="secondary" size={button("sm")} aria-label={t("placeSettings.area.remove", { name })} onClick={() => onChange(removeArea(names, name))}>
                {name}
                <X aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}
      <div className="flex gap-2">
        <Input
          size={size}
          value={typed}
          aria-label={t("placeSettings.area.placeholder")}
          placeholder={t("placeSettings.area.placeholder")}
          onChange={(e) => setTyped(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            e.preventDefault();
            add();
          }}
        />
        <Button type="button" variant="outline" size={button()} disabled={typed.trim() === ""} onClick={add}>
          {t("placeSettings.area.add")}
        </Button>
      </div>
      {errors?.map((reason) => <FieldError key={reason}>{reason}</FieldError>)}
    </section>
  );
}
