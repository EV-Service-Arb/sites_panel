import { Button, Card, CardContent, CardDescription, CardHeader, CardTitle } from "@evinvest/uikit";
import { Calculator } from "lucide-react";

import { type PricingItem, PricingStatus } from "@/entities/pricing";
import { ClearPricingButton } from "@/features/clear-pricing";
import { JsonTransfer, PricingEditorForm, usePricingEditor } from "@/features/edit-pricing";
import { PreviewPanel } from "@/features/preview-price";
import { useT } from "@/shared/i18n";
import { useButtonSize } from "@/shared/ui/touch";

import { PricingHistory } from "./pricing-history";
import { WorkspaceLayout } from "./workspace-layout";

export interface EditWorkspaceProps {
  /** The pricing the draft starts from; re-mount on a new one. */
  base: PricingItem;
  fresher: PricingItem | null;
  today: string;
  version: number;
  /** The person loaded this pricing themselves: the rebuilt screen gives focus to its status. */
  focusStatus: boolean;
  onWritten: (item?: PricingItem) => void;
  onTakeFresh: (item: PricingItem, chosen: boolean) => void;
}

const PREVIEW_ID = "pricing-preview";

/** Below `xl` the preview sits under the whole form: the action bar's way down to it. */
function toPreview() {
  const card = document.getElementById(PREVIEW_ID);
  card?.scrollIntoView({ block: "start", behavior: "smooth" });
  card?.focus({ preventScroll: true });
}

/** An admin's screen: the editor, and the preview of its draft as the server prices it. */
export function EditWorkspace({ base, fresher, today, version, focusStatus, onWritten, onTakeFresh }: EditWorkspaceProps) {
  const t = useT();
  const button = useButtonSize();
  const editor = usePricingEditor(base, today, onWritten);
  return (
    <WorkspaceLayout
      main={
        <>
          <PricingStatus item={base} focusOnMount={focusStatus} action={<ClearPricingButton item={base} onCleared={onWritten} />} />
          <PricingEditorForm
            editor={editor}
            fresher={fresher}
            onTakeFresh={onTakeFresh}
            onReload={() => onWritten()}
            extraAction={
              // A phone's bar holds Save, Discard and this on one line only with the word read, not shown.
              <Button type="button" variant="outline" size={button()} className="ml-auto xl:hidden" onClick={toPreview}>
                <Calculator aria-hidden className="size-4" />
                <span className="max-sm:sr-only">{t("pricing.preview.title")}</span>
              </Button>
            }
          />
        </>
      }
      aside={
        <>
          <PreviewPanel id={PREVIEW_ID} brand={base.brand_id} model={editor.serialized.model} onShowPath={editor.showPath} onShowProblem={editor.showFirst} />
          <Card>
            <CardHeader>
              <CardTitle>{t("pricing.json.title")}</CardTitle>
              <CardDescription>{t("pricing.json.body")}</CardDescription>
            </CardHeader>
            <CardContent>
              <JsonTransfer editor={editor} />
            </CardContent>
          </Card>
          <PricingHistory brand={base.brand_id} version={version} />
        </>
      }
    />
  );
}
