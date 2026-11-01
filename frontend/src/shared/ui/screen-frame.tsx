import { MobileAppBar, PageFrame, type PageFrameWidth } from "@evinvest/uikit";
import type { ReactNode } from "react";

import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";

import { LiveStatusIndicator } from "./live-status";

export interface ScreenFrameProps {
  title: string;
  description?: string;
  /** The screen's controls: beside the heading on desktop, a row under the app bar on a phone. */
  actions?: ReactNode;
  width?: PageFrameWidth;
  children: ReactNode;
}

/**
 * Every screen's frame: the kit's inset and title scale, sections arriving in
 * sequence, the live status beside the heading, on a phone an app bar with the live dot in place of it.
 * The actions are rendered once, wherever the width puts them — a second,
 * CSS-hidden copy would carry its own dialogs and state.
 */
export function ScreenFrame({ title, description, actions, width = "full", children }: ScreenFrameProps) {
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  const appBar = <MobileAppBar title={title} hideFrom="md" right={<LiveStatusIndicator compact />} />;
  const desktopActions = (
    <div className="flex items-center gap-4">
      <LiveStatusIndicator />
      {actions}
    </div>
  );
  return (
    <PageFrame breakpoint="md" width={width} title={title} description={description} actions={isDesktop ? desktopActions : undefined} appBar={appBar}>
      {!isDesktop && actions !== undefined && <div className="flex flex-col gap-3">{actions}</div>}
      {children}
    </PageFrame>
  );
}
