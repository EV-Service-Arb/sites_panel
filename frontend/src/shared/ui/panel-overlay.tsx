import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@evinvest/uikit";
import type { ReactNode } from "react";

import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";

export interface PanelOverlayProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: ReactNode;
  description?: ReactNode;
  /** On desktop: a side sheet (a record read beside its list) or a centred dialog (a short form). */
  desktop: "sheet" | "dialog";
  children: ReactNode;
}

/**
 * One overlay, two shapes: on a phone always the uikit bottom Drawer — reachable
 * with a thumb, dismissed by a drag — and on desktop a Sheet or a Dialog.
 */
export function PanelOverlay({ open, onOpenChange, title, description, desktop, children }: PanelOverlayProps) {
  const isDesktop = useMediaQuery(DESKTOP_QUERY);

  if (!isDesktop) {
    return (
      <Drawer open={open} onOpenChange={onOpenChange}>
        {/* The kit's focus scope falls back to focusing the panel when nothing in it is focusable
            yet (a card still loading), and a div without a tabindex refuses focus: it stayed on the
            row behind the drawer, Tab walked the page, and Escape (heard on the panel) did nothing. */}
        <DrawerContent tabIndex={-1} className="max-h-[90dvh] pb-[env(safe-area-inset-bottom,0px)] outline-none">
          <DrawerHeader>
            <DrawerTitle>{title}</DrawerTitle>
            {description && <DrawerDescription>{description}</DrawerDescription>}
          </DrawerHeader>
          {/* DrawerContent scrolls its own body; a second scroller would fight the drag. */}
          <div className="px-4 pb-4">{children}</div>
        </DrawerContent>
      </Drawer>
    );
  }

  if (desktop === "sheet") {
    return (
      <Sheet open={open} onOpenChange={onOpenChange}>
        <SheetContent side="right" className="w-full overflow-y-auto sm:max-w-md">
          <SheetHeader>
            <SheetTitle>{title}</SheetTitle>
            {description && <SheetDescription>{description}</SheetDescription>}
          </SheetHeader>
          <div className="px-4 pb-6">{children}</div>
        </SheetContent>
      </Sheet>
    );
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        {children}
      </DialogContent>
    </Dialog>
  );
}
