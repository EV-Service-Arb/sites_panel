export { brandsOf, locationsOf } from "./model/place";
export { EDITED_KEYS, settingsToWire, splitLive } from "./model/settings";
export type { EditedFields, PlaceSettings } from "./model/settings";
export type { Place as PlaceSettingsView, PlaceChange as SettingsChange, PlaceSummary as Place } from "@/shared/api/schema";
export { usePlaces, useSelectedBrands } from "./model/use-places";
export { PLACE_TIME_ZONE } from "./config/time-zone";
export { channelPreview, messengerOn } from "./lib/channels";
export type { Channel, ChannelPlan, ChannelSlot } from "./lib/channels";
export { diffLineText, diffSettings, fieldLabel, messengersText } from "./lib/diff";
export type { DiffLine } from "./lib/diff";
export { formatDays, formatHours, isOpenAt, localTimeOf } from "./lib/hours";
export type { LocalTime } from "./lib/hours";
export { ChannelPreview } from "./ui/channel-preview";
export { ChangeEntry } from "./ui/change-entry";
export { ConflictAlert } from "./ui/conflict-alert";
export { StaleAlert } from "./ui/stale-alert";
export { savedSince } from "./lib/live";
export { bookingText } from "./lib/booking";

/** A place as the settings routes name it: brand and slug, both path segments. */
export interface PlaceKey {
  brand: string;
  slug: string;
}
