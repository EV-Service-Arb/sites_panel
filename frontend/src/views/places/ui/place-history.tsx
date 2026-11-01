import { Skeleton } from "@evinvest/uikit";
import { useState } from "react";

import { ChangeEntry, ConflictAlert, type PlaceSettingsView, fetchSettingsHistory, followsPlace } from "@/entities/place";
import { RevertButton } from "@/features/revert-place-change";
import { useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";

export interface PlaceHistoryProps {
  /** The place as last read: its `updated_at` guards each revert. */
  view: PlaceSettingsView;
  version: number;
  onChanged: (view: PlaceSettingsView) => void;
  /** After a 409: read the place again. */
  onReload: () => void;
}

/** The place's changes, newest first, each with a revert for someone who may edit. */
export function PlaceHistory({ view, version, onChanged, onReload }: PlaceHistoryProps) {
  const t = useT();
  const [conflict, setConflict] = useState(false);
  const id = `${view.brand}/${view.slug}`;
  const history = useResource(`history:${id}:${version}`, () => fetchSettingsHistory(view), `history:${id}`, { live: followsPlace(view) });
  if (history.status === "loading") return <Skeleton className="h-32 w-full" />;
  if (history.status === "error") return <ErrorState failure={history.failure} onRetry={history.reload} />;
  if (history.data.length === 0) return <EmptyState className="p-4" title={t("placeSettings.history.empty")} />;
  const revert = (changeId: string) => (
    <RevertButton place={view} changeId={changeId} expectedUpdatedAt={view.updated_at} onReverted={onChanged} onConflict={() => setConflict(true)} />
  );
  return (
    <div className="flex flex-col gap-3">
      {conflict && (
        <ConflictAlert
          onReload={() => {
            setConflict(false);
            onReload();
          }}
        />
      )}
      <ul className="flex flex-col">
        {history.data.map((change) => (
          <ChangeEntry key={change.id} change={change} action={view.can_edit && !conflict ? revert(change.id) : undefined} />
        ))}
      </ul>
    </div>
  );
}
