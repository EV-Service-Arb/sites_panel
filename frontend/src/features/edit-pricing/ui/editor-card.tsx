import { Button, Card, CardAction, CardContent, CardHeader, CardTitle } from "@evinvest/uikit";
import { Trash2 } from "lucide-react";
import type { ReactNode } from "react";

import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

/**
 * The shell every removable editor entry shares. The card is the focus target for jumps from a
 * validation message, hence the id and `tabIndex={-1}`; the remove button names its entry so a
 * screen reader can tell a dozen "Remove" buttons apart.
 */
export function EditorCard({
  id,
  title,
  mono = false,
  removeLabel,
  onRemove,
  children,
}: {
  id: string;
  title: string;
  /** Slug-like titles read better monospaced. */
  mono?: boolean;
  removeLabel: string;
  onRemove: () => void;
  children: ReactNode;
}) {
  const t = useT();
  const button = useButtonSize();
  return (
    <Card id={id} tabIndex={-1} className="outline-none">
      <CardHeader>
        <CardTitle className={mono ? "font-mono wrap-anywhere" : "wrap-anywhere"}>{title}</CardTitle>
        <CardAction>
          <Button type="button" variant="ghost" size={button("sm")} aria-label={t("pricing.field.named", { name: title, field: removeLabel })} onClick={onRemove}>
            <Trash2 aria-hidden className="size-4" />
            {removeLabel}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">{children}</CardContent>
    </Card>
  );
}
