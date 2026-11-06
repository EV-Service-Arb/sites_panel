import type { OpeningHours } from "@evinvest/sites-contract";
import { describe, expect, it } from "vitest";

import { channelPreview } from "@/entities/place/lib/channels";
import { diffLineText, diffSettings } from "@/entities/place/lib/diff";
import { type PlaceSettings, settingsParser, settingsToWire } from "@/entities/place/model/settings";
import { draftChanged, draftOf, editedOf, normaliseBot } from "@/features/edit-place-settings/model/draft";
import { translator } from "@/shared/i18n/translate";
import { parse } from "@/shared/lib/parse";

const en = translator("en");
const settings = (o: unknown): PlaceSettings => parse(settingsParser, o);

describe("the bot and the messenger switches as stored", () => {
  it("are both fields the form edits", () => {
    const s = settings({ telegram: "x_bot", messengers: { telegram: false } });
    expect(s.edited).toEqual({ telegram: "x_bot", messengers: { telegram: false } });
    expect(s.rest).toEqual({});
  });

  it("ride through untouched when a switch is one the panel does not know", () => {
    const s = settings({ phone: "+33423500640", messengers: { signal: true } });
    expect(s.edited).toEqual({ phone: "+33423500640" });
    expect(s.rest).toEqual({ messengers: { signal: true } });
    expect(settingsToWire(s)).toEqual({ phone: "+33423500640", messengers: { signal: true } });
  });

  it("ride through untouched when a switch is not a boolean", () => {
    const s = settings({ messengers: { whatsapp: "off" } });
    expect(s.edited).toEqual({});
    expect(s.rest).toEqual({ messengers: { whatsapp: "off" } });
  });

  it("refuse a bot that is not a string, saying which field", () => {
    expect(() => settings({ telegram: 42 })).toThrow(/^\$\.telegram: /);
  });
});

describe("the form draft with messengers", () => {
  it("saves nothing for a place with every messenger on and no bot", () => {
    expect(editedOf(draftOf({}))).toEqual({});
  });

  it("stores only the switch turned off", () => {
    const draft = draftOf({});
    expect(editedOf({ ...draft, messengers: { whatsapp: true, telegram: false } })).toEqual({ messengers: { telegram: false } });
  });

  it("stores the bot without the @ or the spaces around it", () => {
    expect(normaliseBot("@name_bot ")).toBe("name_bot");
    expect(editedOf({ ...draftOf({}), telegram: "@name_bot " })).toEqual({ telegram: "name_bot" });
  });

  it("is unchanged for a place that stored a switch as on", () => {
    const base = settings({ messengers: { whatsapp: true } });
    expect(draftChanged(draftOf(base.edited), base)).toBe(false);
  });

  it("is changed once a switch is turned off", () => {
    const base = settings({ messengers: { whatsapp: true } });
    expect(draftChanged({ ...draftOf(base.edited), messengers: { whatsapp: false, telegram: true } }, base)).toBe(true);
  });
});

describe("the change history of messengers", () => {
  it("names the bot with its @ and the switches turned off", () => {
    const lines = diffSettings(settings({}), settings({ telegram: "aquafix_bot", messengers: { whatsapp: false, telegram: false } }), en);
    expect(lines.map((l) => diffLineText(l, en))).toEqual(["Telegram bot: set to @aquafix_bot", "Messengers in the lead form: set to WhatsApp, Telegram off"]);
  });

  it("says a switch turned back on as removed", () => {
    const lines = diffSettings(settings({ messengers: { telegram: false } }), settings({}), en);
    expect(lines.map((l) => diffLineText(l, en))).toEqual(["Messengers in the lead form: removed (was Telegram off); the site's own again"]);
  });

  it("finds no difference between no switches and every switch stored as on", () => {
    expect(diffSettings(settings({}), settings({ messengers: { whatsapp: true, telegram: true } }), en)).toEqual([]);
  });
});

describe("the channel preview with messengers", () => {
  const weekdays: OpeningHours = { days: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"], opens: "08:00", closes: "19:00" };
  const fields = { phone: "+33423500640", whatsapp: "+33612345678", telegram: "aquafix_bot", hours: [weekdays] };
  const open = { day: "Monday", minutes: 10 * 60 } as const;
  const closed = { day: "Monday", minutes: 20 * 60 } as const;
  const order = (p: ReturnType<typeof channelPreview>) => p.slots.map((s) => s.channel);

  it("puts Telegram after WhatsApp while open", () => {
    expect(order(channelPreview(fields, open))).toEqual(["call", "whatsapp", "telegram", "callback"]);
  });

  it("puts Telegram after WhatsApp, before the call, while closed", () => {
    expect(order(channelPreview(fields, closed))).toEqual(["callback", "whatsapp", "telegram", "call"]);
  });

  it("names the bot with its @ as the panel's own", () => {
    expect(channelPreview(fields, open).slots.find((s) => s.channel === "telegram")).toEqual({ channel: "telegram", value: "@aquafix_bot", from: "panel" });
  });

  it("leaves out WhatsApp switched off, whatever number the panel has", () => {
    expect(order(channelPreview({ ...fields, messengers: { whatsapp: false } }, open))).toEqual(["call", "telegram", "callback"]);
  });

  it("leaves out Telegram switched off, its bot named or not", () => {
    expect(order(channelPreview({ ...fields, messengers: { telegram: false } }, open))).toEqual(["call", "whatsapp", "callback"]);
  });

  it("lists no Telegram without a bot named in the panel", () => {
    expect(order(channelPreview({ phone: "+33423500640", hours: [weekdays] }, open))).toEqual(["call", "whatsapp", "callback"]);
  });
});
