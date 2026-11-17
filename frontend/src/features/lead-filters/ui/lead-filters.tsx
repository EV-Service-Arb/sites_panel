import { Collapsible, CollapsibleContent, CollapsibleTrigger, Label, Switch, buttonVariants } from "@evinvest/uikit";
import { ChevronDown } from "lucide-react";
import { useId } from "react";

import type { LeadCounts, LeadFilter } from "@/entities/lead";
import { bookingStatusValues, leadChannelValues, leadFlowValues, stageValues, suspectFilterValues } from "@/shared/api/schema";
import { useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { FilterSelect } from "@/shared/ui/filter-select";
import { useButtonSize } from "@/shared/ui/touch";

import { narrows } from "../model/params";
import { RefSearch } from "./ref-search";
import { StageSegments } from "./stage-segments";

type Props = { filter: LeadFilter; brands: string[]; counts: LeadCounts | null; onChange: (patch: Partial<LeadFilter>) => void };

/**
 * Stage, brand, channel, a messenger ref and "overdue only" — the questions the queue is sorted
 * by. On a phone the stages an operator works through are segments under the
 * thumb, and the rest folds under "More filters" (the mockup's hybrid).
 */
export function LeadFilters(props: Props) {
  const t = useT();
  const button = useButtonSize();
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  const { filter, onChange } = props;

  if (isDesktop) {
    return (
      <div className="flex flex-wrap items-center gap-2">
        <FilterSelect
          label={t("filter.stage")}
          allLabel={t("filter.stage.all")}
          value={filter.stage ?? null}
          options={stageValues.map((s) => ({ value: s, label: t(`stage.${s}`) }))}
          onChange={(stage) => onChange({ stage })}
        />
        <SecondaryFilters {...props} />
      </div>
    );
  }

  const more = narrows(filter, ["brand", "overdue", "suspect", "flow", "booking", "channel", "message_ref"]);
  return (
    <div className="flex flex-col gap-2">
      <StageSegments stage={filter.stage ?? null} counts={props.counts} onChange={(stage) => onChange({ stage })} />
      <Collapsible defaultOpen={more} className="flex flex-col gap-2">
        <CollapsibleTrigger className={buttonVariants({ variant: "ghost", size: button("sm"), className: "group self-start px-1" })}>
          {t("filter.more")}
          <ChevronDown aria-hidden className="transition-transform group-aria-expanded:rotate-180" />
        </CollapsibleTrigger>
        <CollapsibleContent className="flex flex-col gap-2">
          <SecondaryFilters {...props} />
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

function SecondaryFilters({ filter, brands, onChange }: Props) {
  const t = useT();
  const id = useId();
  return (
    <>
      <FilterSelect label={t("filter.brand")} allLabel={t("filter.brand.all")} value={filter.brand ?? null} options={brands.map((b) => ({ value: b, label: b }))} onChange={(brand) => onChange({ brand })} />
      <FilterSelect
        label={t("filter.channel")}
        allLabel={t("filter.channel.all")}
        value={filter.channel ?? null}
        options={leadChannelValues.map((c) => ({ value: c, label: t(`channel.${c}`) }))}
        onChange={(channel) => onChange({ channel })}
      />
      <RefSearch value={filter.message_ref ?? null} onChange={(message_ref) => onChange({ message_ref })} />
      <FilterSelect
        label={t("filter.suspect")}
        allLabel={t("filter.suspect.all")}
        value={filter.suspect ?? null}
        options={suspectFilterValues.map((s) => ({ value: s, label: t(`filter.suspect.${s}`) }))}
        onChange={(suspect) => onChange({ suspect })}
      />
      <FilterSelect
        label={t("filter.flow")}
        allLabel={t("filter.flow.all")}
        value={filter.flow ?? null}
        options={leadFlowValues.map((f) => ({ value: f, label: t(`flow.${f}`) }))}
        onChange={(flow) => onChange({ flow })}
      />
      <FilterSelect
        label={t("filter.booking")}
        allLabel={t("filter.booking.all")}
        value={filter.booking ?? null}
        options={bookingStatusValues.map((s) => ({ value: s, label: t(`booking.status.${s}`) }))}
        onChange={(booking) => onChange({ booking })}
      />
      <div className="flex items-center gap-2 px-1 max-md:min-h-11">
        <Switch id={`${id}-overdue`} checked={filter.overdue ?? false} onCheckedChange={(overdue) => onChange({ overdue })} />
        <Label htmlFor={`${id}-overdue`}>{t("filter.overdue")}</Label>
      </div>
    </>
  );
}
