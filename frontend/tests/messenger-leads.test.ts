import { afterEach, describe, expect, it, vi } from "vitest";

import en from "../messages/en.json";
import ru from "../messages/ru.json";

import { LEAD_CHANNELS } from "@evinvest/sites-contract";

import { markMessaged } from "@/entities/lead/api/leads";
import { awaitingMessage, leadParser } from "@/entities/lead/model/lead";
import { leadFilterFrom, narrows, paramsWith } from "@/features/lead-filters/model/params";
import { oneOfOr, parse } from "@/shared/lib/parse";

const row = { brand: "aquafix", lead_id: "l1", stage: "created", manual: false, last_event_at: "2026-10-07T10:00:00Z" };

describe("a messenger lead from the API", () => {
  it("reads a WhatsApp lead with its ref and the Telegram message that answered it", () => {
    const lead = parse(leadParser, { ...row, channel: "whatsapp", message_ref: "AQ-7K3F", messaged_at: "2026-10-07T10:05:00Z", messaged_channel: "telegram" });
    expect(lead).toMatchObject({ channel: "whatsapp", message_ref: "AQ-7K3F", messaged_at: "2026-10-07T10:05:00Z", messaged_channel: "telegram" });
  });

  it("reads a channel newer than this build as other, not as a failed page", () => {
    expect(parse(leadParser, { ...row, channel: "sms" }).channel).toBe("other");
  });

  it("refuses a channel that is not a string, saying which field", () => {
    expect(() => parse(leadParser, { ...row, channel: 42 })).toThrow(/^\$\.channel: /);
  });

  it("reads a messenger newer than this build as none, keeping when the customer wrote", () => {
    const lead = parse(leadParser, { ...row, channel: "whatsapp", messaged_at: "2026-10-07T10:05:00Z", messaged_channel: "phone" });
    expect(lead).toMatchObject({ messaged_at: "2026-10-07T10:05:00Z", messaged_channel: null });
    expect(awaitingMessage(lead)).toBeNull();
  });

  it("reads a lead from before messengers, the fields absent, as none of them", () => {
    const lead = parse(leadParser, row);
    expect(lead).toMatchObject({ channel: null, message_ref: null, messaged_at: null, messaged_channel: null });
  });
});

describe("the messenger a lead waits on", () => {
  it("is WhatsApp for a WhatsApp lead the customer has not written on yet", () => {
    expect(awaitingMessage({ channel: "whatsapp", messaged_at: null })).toBe("whatsapp");
  });

  it("is none once the customer wrote", () => {
    expect(awaitingMessage({ channel: "telegram", messaged_at: "2026-10-07T10:05:00Z" })).toBeNull();
  });

  it("is none for a form lead, a lead without a channel, or a channel newer than this build", () => {
    expect(awaitingMessage({ channel: "form", messaged_at: null })).toBeNull();
    expect(awaitingMessage({ channel: null, messaged_at: null })).toBeNull();
    expect(awaitingMessage({ channel: "other", messaged_at: null })).toBeNull();
  });
});

describe("a word from a list the backend may grow", () => {
  const parser = oneOfOr(["whatsapp", "telegram"], "other");

  it("is itself when the list names it", () => {
    expect(parse(parser, "telegram")).toBe("telegram");
  });

  it("is the fallback for any other string", () => {
    expect(parse(parser, "signal")).toBe("other");
    expect(parse(oneOfOr(["whatsapp"], null), "")).toBeNull();
  });

  it("refuses what is not a string, saying where", () => {
    expect(() => parse(parser, 42)).toThrow(/^\$: expected a string/);
    expect(() => parse(parser, null)).toThrow(/^\$: expected a string/);
  });
});

describe("marking a lead as messaged", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("posts the messenger under the caller's Idempotency-Key", async () => {
    const calls: { url: string; init: RequestInit }[] = [];
    vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
      calls.push({ url, init });
      return new Response(JSON.stringify({ event_id: "e1" }), { status: 201 });
    });
    await markMessaged({ brand: "aquafix", lead: "l1" }, "whatsapp", "key-1");
    expect(calls.map((c) => [c.init.method, c.url])).toEqual([["POST", "/api/sites/leads/aquafix/l1/messaged"]]);
    expect(JSON.parse(String(calls[0]?.init.body))).toEqual({ channel: "whatsapp" });
    expect(new Headers(calls[0]?.init.headers).get("Idempotency-Key")).toBe("key-1");
  });
});

describe("the channel and ref filters in the URL", () => {
  it("drop a channel the API does not take", () => {
    expect(leadFilterFrom(new URLSearchParams("channel=sms")).channel).toBeNull();
  });

  it("go into the URL and back out of it", () => {
    const params = paramsWith(new URLSearchParams("stage=created"), { channel: "telegram", messageRef: "AQ-7K3F" });
    expect(params.toString()).toBe("stage=created&channel=telegram&message_ref=AQ-7K3F");
    expect(leadFilterFrom(params)).toMatchObject({ stage: "created", channel: "telegram", messageRef: "AQ-7K3F" });
    expect(paramsWith(params, { channel: null, messageRef: null }).toString()).toBe("stage=created");
  });

  it("read a lower-case ref from a pasted link as the API matches it", () => {
    expect(leadFilterFrom(new URLSearchParams("message_ref=aq-7k3f")).messageRef).toBe("AQ-7K3F");
  });

  it("each narrow the list on its own", () => {
    const none = leadFilterFrom(new URLSearchParams(""));
    expect(narrows(none)).toBe(false);
    expect(narrows(leadFilterFrom(new URLSearchParams("channel=whatsapp")))).toBe(true);
    expect(narrows(leadFilterFrom(new URLSearchParams("message_ref=AQ-7K3F")))).toBe(true);
    expect(narrows(leadFilterFrom(new URLSearchParams("channel=whatsapp")), ["messageRef"])).toBe(false);
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
