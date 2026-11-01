import { FieldError } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";

import type { Shown } from "../model/errors";
import { messageIdOf } from "../model/fields";
import { shownText } from "./shown-text";

/**
 * The reasons filed under one field, each on its own line. With `field` they
 * carry the ids the field's control names in `aria-describedby`.
 */
export function FieldMessages({ shown, field }: { shown: readonly Shown[] | undefined; field?: string }) {
  const t = useT();
  if (!shown || shown.length === 0) return null;
  return (
    <>
      {shown.map((s, i) => (
        <FieldError key={i} {...(field === undefined ? {} : { id: messageIdOf(field, i) })}>
          {shownText(s, t)}
        </FieldError>
      ))}
    </>
  );
}
