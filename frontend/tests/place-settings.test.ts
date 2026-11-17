import type { OpeningHours } from "@evinvest/sites-contract";
import { describe, expect, it } from "vitest";

import { channelPreview } from "@/entities/place/lib/channels";
import { diffLineText, diffSettings } from "@/entities/place/lib/diff";
import { formatDays, formatHours, localTimeOf } from "@/entities/place/lib/hours";
import { type PlaceSettings, settingsToWire, splitLive } from "@/entities/place/model/settings";
import { addArea, draftChanged, draftOf, draftProblems, editedOf, settingsOf } from "@/features/edit-place-settings/model/draft";
import { fieldErrorsOf } from "@/features/edit-place-settings/model/field-errors";
import { addHoursRow, hoursDraftOf, hoursOf, normaliseTime, removeHoursRow, rowProblems, updateHoursRow } from "@/features/edit-place-settings/model/hours-draft";
import { translator } from "@/shared/i18n/translate";
import type { PlaceLive } from "@/shared/api/schema";

const en = translator("en");

const weekdays: OpeningHours = { days: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"], opens: "08:00", closes: "19:00" };
const address = { street: "12 rue Paul Bert", postalCode: "69003", locality: "Lyon" };

const settings = (live: PlaceLive): PlaceSettings => splitLive(live);

describe("the settings answer", () => {
  const live: PlaceLive = { phone: "+33423500640", hours: [weekdays], serviceArea: ["Royat", "Chamalières"], address, rating: { value: 4.8, count: 31, fetchedAt: "2026-10-01T00:00:00Z" } };

  it("splits the fields the form edits from those it carries through", () => {
    const v = splitLive(live);
    expect(v.edited).toEqual({ phone: "+33423500640", hours: [weekdays], serviceArea: ["Royat", "Chamalières"] });
    expect(Object.keys(v.rest).sort()).toEqual(["address", "rating"]);
  });

  it("sends back the fields it does not edit as they came", () => {
    const v = splitLive(live);
    const draft = draftOf(v.edited);
    draft.phone = "+33 6 12 34 56 78";
    expect(settingsToWire(settingsOf(draft, v))).toEqual({ ...live, phone: "+33612345678" });
  });

  it("splits an empty settings object, a place never set", () => {
    expect(splitLive({})).toEqual({ edited: {}, rest: {} });
  });
});

describe("the hours editor", () => {
  const rows = hoursDraftOf([weekdays, { days: ["Saturday"], opens: "09:00", closes: "12:00" }]);

  it("round-trips the stored rows, days back in week order", () => {
    const shuffled = updateHoursRow(rows, rows[0]?.key ?? -1, { days: ["Friday", "Monday", "Wednesday", "Tuesday", "Thursday"] });
    expect(hoursOf(shuffled)).toEqual([weekdays, { days: ["Saturday"], opens: "09:00", closes: "12:00" }]);
  });

  it("offers a new row the days no row has", () => {
    expect(addHoursRow(rows).at(-1)?.days).toEqual(["Sunday"]);
  });

  it("means the site's own hours once every row is gone", () => {
    const none = rows.reduce((acc, r) => removeHoursRow(acc, r.key), rows);
    expect(hoursOf(none)).toBeUndefined();
  });

  it("takes the times people type", () => {
    expect(["8:00", "0800", "8h30", "08.15", " 9:05 "].map(normaliseTime)).toEqual(["08:00", "08:00", "08:30", "08:15", "09:05"]);
    expect(normaliseTime("25:00")).toBe("25:00");
  });

  it("names what stops a row from saving", () => {
    const [row] = hoursDraftOf([{ days: [], opens: "8", closes: "08:00" }]);
    if (!row) throw new Error("no row");
    expect(rowProblems([row], row).map((p) => p.code)).toEqual(["empty", "time"]);
    const same = { ...row, days: ["Monday" as const], opens: "08:00" };
    expect(rowProblems([same], same).map((p) => p.code)).toEqual(["closes_before_opens"]);
  });

  it("allows a lunch break, refuses rows whose times overlap on a day", () => {
    const split = hoursDraftOf([
      { days: ["Monday"], opens: "08:00", closes: "12:00" },
      { days: ["Monday", "Tuesday"], opens: "14:00", closes: "19:00" },
    ]);
    expect(split.flatMap((r) => rowProblems(split, r))).toEqual([]);
    const clash = updateHoursRow(split, split[1]?.key ?? -1, { opens: "11:00" });
    expect(rowProblems(clash, clash[0] ?? split[0]!)).toEqual([{ code: "overlap", days: ["Monday"] }]);
  });

  it("refuses a row past midnight, as the server does: a night is two rows", () => {
    const night = hoursDraftOf([
      { days: ["Friday"], opens: "20:00", closes: "02:00" },
      { days: ["Saturday"], opens: "01:00", closes: "05:00" },
    ]);
    expect(rowProblems(night, night[0]!)).toEqual([{ code: "closes_before_opens" }]);
    expect(rowProblems(night, night[1]!)).toEqual([]);
  });
});

describe("the form draft", () => {
  it("is unchanged until a field is", () => {
    const base = settings({ phone: "+33612345678", hours: [{ days: ["Tuesday", "Monday"], opens: "08:00", closes: "19:00" }], address });
    const draft = draftOf(base.edited);
    expect(draftChanged(draft, base)).toBe(false);
    expect(draftChanged({ ...draft, whatsapp: "+33600000000" }, base)).toBe(true);
  });

  it("drops an emptied field, so the site's own value comes back", () => {
    const draft = draftOf({ phone: "+33612345678", serviceArea: ["Royat"] });
    expect(editedOf({ ...draft, phone: "  ", serviceArea: [] })).toEqual({});
  });

  it("refuses a number that is not E.164 on its field, never an empty one", () => {
    const problems = (phone: string) => [...draftProblems({ ...draftOf({}), phone })];
    expect(["+33 6 12 34 56 78", "+33612345678", ""].map(problems)).toEqual([[], [], []]);
    expect(["0612345678", "+0612345678", "+33"].map(problems)).toEqual([[["phone", "e164"]], [["phone", "e164"]], [["phone", "e164"]]]);
  });

  it("adds a commune once, whatever its case or spacing", () => {
    expect(addArea(["Royat"], "  royat ")).toEqual(["Royat"]);
    expect(addArea(["Royat"], " Chamalières  sur  Loire ")).toEqual(["Royat", "Chamalières sur Loire"]);
    expect(addArea(["Royat"], "   ")).toEqual(["Royat"]);
  });
});

describe("the form's own problems", () => {
  it("are told field by field, a path inside a field on that field", () => {
    const sorted = fieldErrorsOf({ phone: "must be E.164", "hours[0].opens": "must be HH:MM" });
    expect(sorted.byField).toEqual({ phone: ["must be E.164"], hours: ["must be HH:MM"] });
  });

  it("name a field the form edits, or the form is wrong", () => {
    expect(() => fieldErrorsOf({ geo: "out of range" })).toThrow(/geo/);
  });
});

describe("the change history in plain words", () => {
  it("says what was set, changed and removed, and nothing about what stayed", () => {
    const before = settings({ phone: "+33100000000", hours: [weekdays], address });
    const after = settings({ phone: "+33200000000", whatsapp: "+33600000000", address });
    expect(diffSettings(before, after, en).map((l) => diffLineText(l, en))).toEqual([
      "Phone: +33100000000 → +33200000000",
      "WhatsApp: set to +33600000000",
      "Opening hours: removed (was Mon–Fri 08:00–19:00); the site's own again",
    ]);
  });

  it("names a field the panel does not edit by its wire name", () => {
    const lines = diffSettings(settings({ address }), settings({}), en);
    expect(lines).toEqual([{ field: "address", before: JSON.stringify(address), after: null }]);
  });

  it("finds no difference between equal settings", () => {
    expect(diffSettings(settings({ serviceArea: ["Royat"] }), settings({ serviceArea: ["Royat"] }), en)).toEqual([]);
  });

  it("writes days as runs, short runs one by one", () => {
    expect(formatDays(["Saturday", "Monday", "Tuesday", "Wednesday"], en)).toBe("Mon–Wed, Sat");
    expect(formatDays(["Monday", "Tuesday", "Sunday"], en)).toBe("Mon, Tue, Sun");
    expect(formatHours([weekdays, { days: ["Saturday"], opens: "09:00", closes: "12:00" }], translator("ru"))).toBe("Пн–Пт 08:00–19:00; Сб 09:00–12:00");
  });
});

describe("the channel preview", () => {
  const hours = [weekdays];
  const fields = { phone: "+33423500640", whatsapp: "+33612345678", hours };
  const order = (p: ReturnType<typeof channelPreview>) => p.slots.map((s) => s.channel);

  it("puts the call first while open", () => {
    const p = channelPreview(fields, { day: "Monday", minutes: 10 * 60 });
    expect(p.state).toBe("open");
    expect(order(p)).toEqual(["call", "whatsapp", "callback"]);
  });

  it("puts the callback and WhatsApp first while closed, the call last", () => {
    const p = channelPreview(fields, { day: "Monday", minutes: 19 * 60 });
    expect(p.state).toBe("closed");
    expect(order(p)).toEqual(["callback", "whatsapp", "call"]);
    expect(channelPreview(fields, { day: "Sunday", minutes: 10 * 60 }).state).toBe("closed");
  });

  it("does not guess open or closed on the site's own hours", () => {
    expect(channelPreview({ phone: "+33423500640" }, { day: "Monday", minutes: 3 * 60 }).state).toBe("unknown");
  });

  it("never fills in a number the panel does not have", () => {
    const p = channelPreview({ hours }, { day: "Monday", minutes: 10 * 60 });
    expect(p.slots.find((s) => s.channel === "call")).toEqual({ channel: "call", value: null, from: "site" });
    expect(p.slots.find((s) => s.channel === "whatsapp")).toEqual({ channel: "whatsapp", value: null, from: "site" });
  });

  it("tells the time on the place's clock, not the browser's", () => {
    // 2026-10-05 is a Monday; 06:30 UTC is 08:30 in Paris (summer time).
    expect(localTimeOf(new Date("2026-10-05T06:30:00Z"), "Europe/Paris")).toEqual({ day: "Monday", minutes: 8 * 60 + 30 });
    expect(localTimeOf(new Date("2026-10-04T23:30:00Z"), "Europe/Paris")).toEqual({ day: "Monday", minutes: 90 });
  });
});
