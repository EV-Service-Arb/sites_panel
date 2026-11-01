import { cn } from "@evinvest/uikit";

import { useT } from "@/shared/i18n";
import { type LiveStatus, useLiveStatus } from "@/shared/lib/live";

type Shown = "live" | "reconnecting" | "offline";

/** Paused is a hidden tab and connecting the first second: both read as "getting there". Null: the host has not said yet. */
function shownOf(status: LiveStatus | null): Shown | null {
  switch (status) {
    case "live":
      return "live";
    case "offline":
      return "offline";
    case null:
    case "closed":
      return null;
    default:
      return "reconnecting";
  }
}

const DOT: Record<Shown, string> = {
  live: "bg-positive",
  reconnecting: "bg-accent-warn motion-safe:animate-pulse",
  offline: "bg-accent-error",
};

/**
 * Whether the screens follow changes as they happen. `compact` (a phone's app
 * bar) keeps only the dot while all is well and says so in words otherwise.
 * A polite live region: a screen reader hears the connection drop and return.
 */
export function LiveStatusIndicator({ compact = false, className }: { compact?: boolean; className?: string }) {
  const t = useT();
  const shown = shownOf(useLiveStatus());
  if (shown === null) return null;
  const text = t(`live.status.${shown}`);
  return (
    <p role="status" aria-live="polite" title={t(`live.status.${shown}.hint`)} className={cn("flex items-center gap-2 text-xs text-ink-soft", className)}>
      <span aria-hidden className={cn("size-2 shrink-0 rounded-full", DOT[shown])} />
      <span className={cn(compact && shown === "live" && "sr-only")}>{text}</span>
    </p>
  );
}
