import { messageRefOf } from "@evinvest/sites-contract";
import { InputGroup, InputGroupAddon, InputGroupInput, cn } from "@evinvest/uikit";
import { Search } from "lucide-react";
import { useState } from "react";

import { useT } from "@/shared/i18n";
import { useControlSize } from "@/shared/ui/touch";

/**
 * The messenger ref a customer quoted ("Réf. AQ-7K3F"), matched exactly in any
 * case. Applied on Enter or leaving the field, so a half-typed ref does not
 * re-read the list on every key; one that cannot be a ref is said, not sent.
 */
export function RefSearch({ value, onChange }: { value: string | null; onChange: (ref: string | null) => void }) {
  const t = useT();
  const size = useControlSize();
  const [text, setText] = useState(value ?? "");
  const [seen, setSeen] = useState(value);
  // The URL changed under the field (cleared filters, a link): show what it holds.
  if (seen !== value) {
    setSeen(value);
    setText(value ?? "");
  }
  const blank = text.trim() === "";
  const ref = messageRefOf(text);
  const invalid = !blank && ref === null;
  const apply = () => {
    const next = blank ? null : ref;
    if ((blank || ref !== null) && next !== value) onChange(next);
  };

  return (
    // The height of the selects beside it: theirs is `sm` on a desktop, `lg` under the thumb.
    <InputGroup className={cn("w-40 max-md:w-full", size === "lg" ? "h-12 text-base" : "h-8")} data-invalid={invalid ? true : undefined}>
      <InputGroupAddon>
        <Search aria-hidden />
      </InputGroupAddon>
      <InputGroupInput
        aria-label={t("filter.ref")}
        aria-invalid={invalid ? true : undefined}
        title={invalid ? t("filter.ref.invalid") : undefined}
        placeholder={t("filter.ref")}
        autoCapitalize="characters"
        autoComplete="off"
        spellCheck={false}
        className="font-mono uppercase placeholder:font-sans placeholder:normal-case"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={apply}
        onKeyDown={(e) => {
          if (e.key === "Enter") apply();
        }}
      />
    </InputGroup>
  );
}
