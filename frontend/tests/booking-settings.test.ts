import { BOOKING_CODES, PLACE_CODES, checkBookingConfig } from "@evinvest/sites-contract";
import { describe, expect, it } from "vitest";

import { bookingText } from "@/entities/place/lib/booking";
import { diffLineText, diffSettings } from "@/entities/place/lib/diff";
import { settingsToWire, splitLive } from "@/entities/place/model/settings";
import { bookingDraftOf, bookingOf } from "@/features/edit-place-settings/model/booking-draft";
import { draftChanged, draftOf, draftProblems, draftValid, editedOf, settingsOf } from "@/features/edit-place-settings/model/draft";
import { fieldErrorsOf } from "@/features/edit-place-settings/model/field-errors";
import { translator } from "@/shared/i18n/translate";

const en = translator("en");

describe("a place's booking setting", () => {
  const booking = { default: "google_calendar" as const, providers: { google_calendar: { url: "https://calendar.app.google/AbC" }, link: { url: "https://book.example.fr/v" } } };

  it("is a field the form edits, and goes back as it came", () => {
    const s = splitLive({ phone: "+33612345678", booking });
    expect(s.edited.booking).toEqual(booking);
    expect(s.rest).toEqual({});
    expect(settingsToWire(settingsOf(draftOf(s.edited), s))).toEqual({ phone: "+33612345678", booking: { default: "google_calendar", providers: { link: booking.providers.link, google_calendar: booking.providers.google_calendar } } });
  });

  it("stays untouched in what the form carries when the server would now refuse it", () => {
    const odd = { default: "link" as const, providers: {} };
    const s = splitLive({ booking: odd });
    expect(s.edited.booking).toBeUndefined();
    expect(s.rest).toEqual({ booking: odd });
  });

  it("an untouched form is unchanged, whatever order the server keeps the providers in", () => {
    const s = splitLive({ booking });
    expect(draftChanged(draftOf(s.edited), s)).toBe(false);
  });

  it("reads in the history in plain words", () => {
    const { config } = checkBookingConfig(booking);
    if (config === null) throw new Error("the fixture is a valid booking");
    const text = "Default: Google Calendar; Booking link: https://book.example.fr/v; Google Calendar: https://calendar.app.google/AbC";
    expect(bookingText(config, en)).toBe(text);
    const lines = diffSettings(splitLive({}), splitLive({ booking }), en).map((l) => diffLineText(l, en));
    expect(lines).toEqual([`Booking: set to ${text}`]);
  });
});

describe("the booking block of the form", () => {
  const empty = bookingDraftOf(undefined);

  it("sends nothing while nothing is chosen: the site keeps its own", () => {
    expect(bookingOf(empty)).toBeNull();
    expect(draftProblems({ ...draftOf({}), booking: empty }).size).toBe(0);
    expect(editedOf(draftOf({})).booking).toBeUndefined();
  });

  it("asks for a default once a page is typed", () => {
    const d = { ...empty, urls: { ...empty.urls, link: "https://book.example.fr/v" } };
    expect(Object.fromEntries(draftProblems({ ...draftOf({}), booking: d }))).toEqual({ "booking.default": "default_choose" });
    expect(draftValid({ ...draftOf({}), booking: d })).toBe(false);
  });

  it("trims what was pasted and leaves an empty page out", () => {
    const d = { default: "cal_com" as const, urls: { link: "  ", google_calendar: "", cal_com: " https://cal.com/vifnet/menage " } };
    expect(bookingOf(d)).toEqual({ default: "cal_com", providers: { cal_com: { url: "https://cal.com/vifnet/menage" } } });
    expect(draftProblems({ ...draftOf({}), booking: d }).size).toBe(0);
  });

  it("keys its problems as the server's 422 does, so both land on the same field", () => {
    const d = { default: "google_calendar" as const, urls: { link: "http://x.fr", google_calendar: "", cal_com: "" } };
    expect(Object.fromEntries(draftProblems({ ...draftOf({}), booking: d }))).toEqual({ "booking.providers.link.url": "scheme", "booking.default": "default_unset" });
  });

  it("sorts a 422 on booking under the booking field, every key kept", () => {
    const sorted = fieldErrorsOf({ "booking.default": "must be manual or one of the providers set", "booking.providers.google_calendar.url": "must be a Google appointment schedule" });
    expect(sorted.byField.booking).toHaveLength(2);
    expect(sorted.byKey["booking.providers.google_calendar.url"]).toBe("must be a Google appointment schedule");
  });

  it("has words for every problem in both languages", () => {
    const ru = translator("ru");
    for (const p of [...PLACE_CODES, ...BOOKING_CODES, "default_choose"] as const) {
      expect(en(`placeSettings.problem.${p}`)).not.toContain("placeSettings");
      expect(ru(`placeSettings.problem.${p}`)).not.toBe(en(`placeSettings.problem.${p}`));
    }
  });
});
