import { Card, CardContent, CardHeader, CardTitle, Table, TableBody, TableCell, TableHead, TableHeader, TableRow, cn } from "@evinvest/uikit";

import type { Experiment } from "@/entities/experiment";
import { useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { EDGE_CELL, TABLE_HEAD } from "@/shared/ui/table";

import { ChangeLines, Controls, KeyCell, PostHogLink, SplitList, StatusBadge } from "./experiment-cells";

const HEAD = cn(TABLE_HEAD, EDGE_CELL);

interface Props {
  brand: string;
  experiments: Experiment[];
  /** An admin's: the switch and the weights in each row. */
  editable: boolean;
  onSaved: () => void;
}

/**
 * One brand's experiments in a card: a table from `md` up, stacked rows on a
 * phone, where a sideways-scrolling table would hide the switch. Retired ones
 * stay, dimmed, so a link to PostHog for a finished test is still a click away.
 */
export function BrandExperiments({ brand, experiments, editable, onSaved }: Props) {
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  return (
    <Card className="gap-0 overflow-hidden py-0">
      <CardHeader className="px-5 py-4">
        <CardTitle className="text-base text-ink">{brand}</CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        {isDesktop ? <Wide experiments={experiments} editable={editable} onSaved={onSaved} /> : <Narrow experiments={experiments} editable={editable} onSaved={onSaved} />}
      </CardContent>
    </Card>
  );
}

type ListProps = Omit<Props, "brand">;

function Wide({ experiments, editable, onSaved }: ListProps) {
  const t = useT();
  return (
    <Table>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead className={HEAD}>{t("experiments.col.key")}</TableHead>
          <TableHead className={HEAD}>{t("experiments.col.split")}</TableHead>
          <TableHead className={HEAD}>{t("experiments.col.status")}</TableHead>
          <TableHead className={HEAD}>{t("experiments.col.changes")}</TableHead>
          <TableHead className={HEAD}>
            <span className="sr-only">{t("experiments.col.actions")}</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {experiments.map((e) => (
          <TableRow key={e.key} className={cn("align-top", e.retired && "opacity-60")}>
            <TableCell className={cn(EDGE_CELL, "max-w-xs whitespace-normal")}>
              <KeyCell experiment={e} />
            </TableCell>
            <TableCell className={cn(EDGE_CELL, "min-w-40")}>
              <SplitList experiment={e} />
            </TableCell>
            <TableCell className={EDGE_CELL}>
              <StatusBadge experiment={e} />
            </TableCell>
            <TableCell className={EDGE_CELL}>
              <ChangeLines experiment={e} />
            </TableCell>
            <TableCell className={cn(EDGE_CELL, "text-right")}>
              <div className="flex flex-col items-end gap-2">
                {editable && <Controls experiment={e} onSaved={onSaved} />}
                <PostHogLink experiment={e} />
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function Narrow({ experiments, editable, onSaved }: ListProps) {
  return (
    <ul className="divide-y divide-border">
      {experiments.map((e) => (
        <li key={e.key} className={cn("flex flex-col gap-3 px-4 py-3", e.retired && "opacity-60")}>
          <div className="flex items-start justify-between gap-3">
            <KeyCell experiment={e} />
            <StatusBadge experiment={e} />
          </div>
          <SplitList experiment={e} />
          <ChangeLines experiment={e} />
          <div className="flex flex-wrap items-center justify-between gap-3">
            {editable && <Controls experiment={e} onSaved={onSaved} />}
            <PostHogLink experiment={e} />
          </div>
        </li>
      ))}
    </ul>
  );
}
