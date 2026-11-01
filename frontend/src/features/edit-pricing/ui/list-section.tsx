import { Button } from "@evinvest/uikit";
import { Plus } from "lucide-react";
import { type ReactNode, useId } from "react";

import { EmptyState } from "@/shared/ui/empty-state";
import { useButtonSize } from "@/shared/ui/touch";

import type { Shown } from "../model/errors";
import { domIdOf } from "../model/fields";
import { FieldMessages } from "./field-messages";

export interface ListSectionProps {
  /** The list's field (`inputs`, `needs`): its own reasons, and where a link to it lands. */
  field: string;
  title: string;
  description: string;
  /** Said in place of the rows while there are none. */
  empty: string | null;
  shown: readonly Shown[] | undefined;
  add: { label: string; onAdd: () => void };
  children: ReactNode;
}

/** A list of the model's rows (questions, needs): heading, its reasons, the rows, and a way to add one. */
export function ListSection({ field, title, description, empty, shown, add, children }: ListSectionProps) {
  const button = useButtonSize();
  const titleId = useId();
  return (
    <section id={domIdOf(field)} tabIndex={-1} className="flex flex-col gap-3 outline-none" aria-labelledby={titleId}>
      <div className="flex flex-col gap-1">
        <h2 id={titleId} className="text-lg font-semibold text-ink">
          {title}
        </h2>
        <p className="text-sm text-ink-soft">{description}</p>
      </div>
      <FieldMessages shown={shown} />
      {empty !== null && <EmptyState className="p-4" title={empty} />}
      {children}
      <Button type="button" variant="outline" size={button()} className="self-start" onClick={add.onAdd}>
        <Plus aria-hidden className="size-4" />
        {add.label}
      </Button>
    </section>
  );
}
