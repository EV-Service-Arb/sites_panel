import { type BookingConfig, type BookingProvider, PAGE_PROVIDERS, type PageProvider } from "@evinvest/sites-contract";

/** The booking block as typed. No default chosen and no page typed: the site's own booking. */
export interface BookingDraft {
  default: BookingProvider | null;
  urls: Record<PageProvider, string>;
}

export function bookingDraftOf(b: BookingConfig | undefined): BookingDraft {
  return {
    default: b?.default ?? null,
    urls: { link: b?.providers.link?.url ?? "", google_calendar: b?.providers.google_calendar?.url ?? "", cal_com: b?.providers.cal_com?.url ?? "" },
  };
}

/** The setting to save, or null for none (the site's own). Each page trimmed, empty ones left out. */
export function bookingOf(d: BookingDraft): BookingConfig | null {
  const pages = PAGE_PROVIDERS.filter((p) => d.urls[p].trim() !== "");
  if (d.default === null && pages.length === 0) return null;
  const providers: BookingConfig["providers"] = {};
  for (const p of pages) providers[p] = { url: d.urls[p].trim() };
  return { default: d.default ?? "manual", providers };
}

/** The 422 key of a provider's page. */
export const urlKey = (p: PageProvider) => `booking.providers.${p}.url`;
