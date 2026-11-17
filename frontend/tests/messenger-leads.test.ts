import { describe, expect, it } from "vitest";

import en from "../messages/en.json";
import ru from "../messages/ru.json";

import { LEAD_CHANNELS } from "@evinvest/sites-contract";

import { awaitingMessage } from "@/entities/lead/model/lead";

import { lead as leadOf } from "./fixtures";

describe("the messenger a lead waits on", () => {
  it("is WhatsApp for a WhatsApp lead the customer has not written on yet", () => {
    expect(awaitingMessage({ channel: "whatsapp", messaged_at: null })).toBe("whatsapp");
  });

  it("is none once the customer wrote", () => {
    expect(awaitingMessage({ channel: "telegram", messaged_at: "2026-10-07T10:05:00Z" })).toBeNull();
  });

  it("is none for a form lead or a lead without a channel", () => {
    expect(awaitingMessage({ channel: "form", messaged_at: null })).toBeNull();
    expect(awaitingMessage({ channel: null, messaged_at: null })).toBeNull();
  });

  it("is none once the customer wrote on the other messenger", () => {
    expect(awaitingMessage(leadOf({ channel: "whatsapp", messaged_at: "2026-10-07T10:05:00Z", messaged_channel: "telegram" }))).toBeNull();
  });
});

describe("the channel badge's words", () => {
  const key = (channel: string) => `channel.${channel}`;
  const missing = (catalogue: Record<string, string>) => LEAD_CHANNELS.map(key).filter((k) => !(k in catalogue));

  it("exist in English for every channel the contract names", () => {
    expect(missing(en)).toEqual([]);
  });

  it("exist in Russian for every channel the contract names", () => {
    expect(missing(ru)).toEqual([]);
  });
});
