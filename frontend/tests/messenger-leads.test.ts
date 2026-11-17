import { describe, expect, it } from "vitest";

import en from "../messages/en.json";
import ru from "../messages/ru.json";

import { LEAD_CHANNELS } from "@evinvest/sites-contract";

import { awaitingMessage } from "@/entities/lead/model/lead";
import { leadFilterFrom, narrows, paramsWith } from "@/features/lead-filters/model/params";

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

describe("the channel and ref filters in the URL", () => {
  it("drop a channel the API does not take", () => {
    expect(leadFilterFrom(new URLSearchParams("channel=sms")).channel).toBeNull();
  });

  it("go into the URL and back out of it", () => {
    const params = paramsWith(new URLSearchParams("stage=created"), { channel: "telegram", message_ref: "AQ-7K3F" });
    expect(params.toString()).toBe("stage=created&channel=telegram&message_ref=AQ-7K3F");
    expect(leadFilterFrom(params)).toMatchObject({ stage: "created", channel: "telegram", message_ref: "AQ-7K3F" });
    expect(paramsWith(params, { channel: null, message_ref: null }).toString()).toBe("stage=created");
  });

  it("read a lower-case ref from a pasted link as the API matches it", () => {
    expect(leadFilterFrom(new URLSearchParams("message_ref=aq-7k3f")).message_ref).toBe("AQ-7K3F");
  });

  it("each narrow the list on its own", () => {
    const none = leadFilterFrom(new URLSearchParams(""));
    expect(narrows(none)).toBe(false);
    expect(narrows(leadFilterFrom(new URLSearchParams("channel=whatsapp")))).toBe(true);
    expect(narrows(leadFilterFrom(new URLSearchParams("message_ref=AQ-7K3F")))).toBe(true);
    expect(narrows(leadFilterFrom(new URLSearchParams("channel=whatsapp")), ["message_ref"])).toBe(false);
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
