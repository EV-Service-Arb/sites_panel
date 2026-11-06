import { BOOKING_STATUSES, LEAD_CHANNELS, LEAD_FLOWS } from "@evinvest/sites-contract";
import { Collapsible, CollapsibleContent, CollapsibleTrigger, Label, Switch, buttonVariants } from "@evinvest/uikit";
import { ChevronDown } from "lucide-react";
import { useId } from "react";

import { type LeadCounts, type LeadFilter, STAGES } from "@/entities/lead";
import { useT } from "@/shared/i18n";
import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";
import { FilterSelect } from "@/shared/ui/filter-select";
import { useButtonSize } from "@/shared/ui/touch";

import { bookingFilterOf, channelFilterOf, flowFilterOf, narrows, stageOrNull, suspectFilterOf } from "../model/params";
import { RefSearch } from "./ref-search";
import { StageSegments } from "./stage-segments";

type Props = { filter: LeadFilter; brands: string[]; locations: string[]; counts: LeadCounts | null; onChange: (patch: Partial<LeadFilter>) => void };

/**
 * Stage, brand, location, channel, a messenger ref and "overdue only" — the questions the queue is sorted
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
          value={filter.stage}
          options={STAGES.map((s) => ({ value: s, label: t(`stage.${s}`) }))}
          onChange={(v) => onChange({ stage: stageOrNull(v) })}
        />
        <SecondaryFilters {...props} />
      </div>
    );
  }

  const more = narrows(filter, ["brand", "location", "overdue", "suspect", "flow", "booking", "channel", "messageRef"]);
  return (
    <div className="flex flex-col gap-2">
      <StageSegments stage={filter.stage} counts={props.counts} onChange={(stage) => onChange({ stage })} />
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

function SecondaryFilters({ filter, brands, locations, onChange }: Props) {
  const t = useT();
  const id = useId();
  return (
    <>
      <FilterSelect label={t("filter.brand")} allLabel={t("filter.brand.all")} value={filter.brand} options={brands.map((b) => ({ value: b, label: b }))} onChange={(brand) => onChange({ brand, location: null })} />
      <FilterSelect label={t("filter.location")} allLabel={t("filter.location.all")} value={filter.location} options={locations.map((l) => ({ value: l, label: l }))} onChange={(location) => onChange({ location })} />
      <FilterSelect
        label={t("filter.channel")}
        allLabel={t("filter.channel.all")}
        value={filter.channel}
        options={LEAD_CHANNELS.map((c) => ({ value: c, label: t(`channel.${c}`) }))}
        onChange={(v) => onChange({ channel: channelFilterOf(v) })}
      />
      <RefSearch value={filter.messageRef} onChange={(messageRef) => onChange({ messageRef })} />
      <FilterSelect
        label={t("filter.suspect")}
        allLabel={t("filter.suspect.all")}
        value={filter.suspect}
        options={[
          { value: "only", label: t("filter.suspect.only") },
          { value: "exclude", label: t("filter.suspect.exclude") },
        ]}
        onChange={(v) => onChange({ suspect: suspectFilterOf(v) })}
      />
      <FilterSelect
        label={t("filter.flow")}
        allLabel={t("filter.flow.all")}
        value={filter.flow}
        options={LEAD_FLOWS.map((f) => ({ value: f, label: t(`flow.${f}`) }))}
        onChange={(v) => onChange({ flow: flowFilterOf(v) })}
      />
      <FilterSelect
        label={t("filter.booking")}
        allLabel={t("filter.booking.all")}
        value={filter.booking}
        options={BOOKING_STATUSES.map((s) => ({ value: s, label: t(`booking.status.${s}`) }))}
        onChange={(v) => onChange({ booking: bookingFilterOf(v) })}
      />
      <div className="flex items-center gap-2 px-1 max-md:min-h-11">
        <Switch id={`${id}-overdue`} checked={filter.overdue} onCheckedChange={(overdue) => onChange({ overdue })} />
        <Label htmlFor={`${id}-overdue`}>{t("filter.overdue")}</Label>
      </div>
    </>
  );
}
