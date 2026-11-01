import { Card, CardContent } from "@evinvest/uikit";

import { ModelSummary, type PricingItem, PricingStatus } from "@/entities/pricing";
import { PreviewPanel } from "@/features/preview-price";
import { useT } from "@/shared/i18n";
import { EmptyState } from "@/shared/ui/empty-state";

import { PricingHistory } from "./pricing-history";
import { WorkspaceLayout } from "./workspace-layout";

const nowhere = () => undefined;

/** An operator's screen: the model as saved, and the same preview to check a quote against. */
export function ReadWorkspace({ item, version }: { item: PricingItem; version: number }) {
  const t = useT();
  return (
    <WorkspaceLayout
      main={
        <>
          <PricingStatus item={item} />
          {item.model ? (
            <Card>
              <CardContent>
                <ModelSummary model={item.model} />
              </CardContent>
            </Card>
          ) : (
            <EmptyState title={t("pricing.readonly.none")} description={t("pricing.readonly.noneBody")} />
          )}
        </>
      }
      aside={
        <>
          {/* A saved model has no field to show: a refusal here is the server's own, told as it came. */}
          {item.model && <PreviewPanel brand={item.brand_id} model={item.model} onShowPath={nowhere} />}
          <PricingHistory brand={item.brand_id} version={version} />
        </>
      }
    />
  );
}
