import type { ButtonSize } from "@evinvest/uikit";

import { DESKTOP_QUERY, useMediaQuery } from "@/shared/lib/use-media-query";

/**
 * A Button's size: the kit's `touch` (44px minimum, WCAG 2.5.5 / Apple HIG) on a
 * phone, the given size from `md` up. One call per component, then
 * `size={button("lg")}` — the hook stays out of loops and conditions.
 */
export function useButtonSize(): (desktop?: ButtonSize) => ButtonSize {
  const isDesktop = useMediaQuery(DESKTOP_QUERY);
  return (desktop = "md") => (isDesktop ? desktop : "touch");
}

/** Input and SelectTrigger: the kit's `lg` (48px) on a phone, `md` from `md` up. */
export function useControlSize(): "md" | "lg" {
  return useMediaQuery(DESKTOP_QUERY) ? "md" : "lg";
}
