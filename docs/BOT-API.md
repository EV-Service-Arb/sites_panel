# Bot API

What a messenger bot or auto-responder (a brand's Telegram bot, a WhatsApp responder) sends to
the sites and asks of them: a lead that started as a conversation, the fact that a customer
actually wrote, and the lead a messenger ref names (MESSENGER-CHANNELS-SPEC §3). It is the
ingest API with a source of kind `bot`: the same signature, the same contract
([`contracts/proto/sa/v1/events.proto`](../contracts/proto/sa/v1/events.proto)), in-cluster
only (the public ingress does not route `/api/ingest`). The conductor (site_conductor) holds the
keys and checks the signature; sites_panel decides what the key may write and answers.

## A key

An admin issues one per bot at the conductor, for the brands it answers for (its Sources page,
kind "bot"). The secret is shown once.

A bot's key may write `lead.created` (channel `whatsapp` or `telegram` only) and
`lead.messaged`, for its brands; anything else is `rejected`. It is the only kind of key that
may look a lead up by its ref.

## Signing

Every request carries three headers:

```text
x-sa-key-id:    aquafix-tg
x-sa-timestamp: 1791370800                     unix seconds; ±5 minutes of the conductor's clock
x-sa-signature: hex(HMAC-SHA256(secret, "sa-ingest/v1." + <x-sa-timestamp> + "." + <raw body>))
```

The body is the bytes sent, exactly. A `GET` (the lookup) has none: its MAC is over the method
and the path as sent, with its query if any — `sa-ingest/v1.<timestamp>.GET
/api/ingest/v1/leads/by-ref/aquafix/AQ-7K3F` — so a signature opens that one lookup, for its
five minutes, and no other. A bad key or signature is `401 {"error": "invalid key or
signature"}`, a timestamp outside the window `401 {"error": "timestamp outside the replay
window"}`.

## Sending events

`POST /api/ingest/v1/events` with `{"events": [ … ]}`, 1 to 500 events; the answer is `207`
with a verdict per event, in order: `accepted`, `duplicate` (that id is journaled already),
`rejected` with a `reason`, or `deferred` with a `reason` — not journaled yet, because it names
what has not arrived. A batch holding a `deferred` event is answered `409` with `Retry-After`
(seconds) and the same verdicts: send the whole batch again after that delay; what was accepted
the first time comes back `duplicate`. Every event has a fresh UUIDv7 `id` (the idempotency key: send the
same event again after a timeout, not a new one), `"schema": "sa.funnel.v1"`, `"typeVersion":
1`, `occurredAt` (RFC 3339), `"source": {"kind": "bot", "id": <the key id>}`.

The customer's handle — a Telegram `@username`, a WhatsApp number or display name — is PII: it
goes in `pii` (`handle`, `phone`, `name`), which the panel seals, never in `properties` or in an
id. Ids are stored in the clear and refused when they look like a phone number.

### A new conversation, without the site: `lead.created`

The customer wrote first, with no ref (or one the panel does not know). The bot makes the
lead's id itself — unique within the brand, `wa-…` / `tg-…` and something random or a hash of
the chat id, never the number itself:

```json
{
  "id": "0192f1c2-7d1e-7b3a-9c4d-1a2b3c4d5e6f",
  "schema": "sa.funnel.v1",
  "type": "lead.created",
  "typeVersion": 1,
  "occurredAt": "2026-10-07T09:12:00Z",
  "source": {"kind": "bot", "id": "aquafix-tg"},
  "subject": {"brandId": "aquafix", "locationId": "royat", "leadId": "tg-5f1c9a2e"},
  "properties": {"channel": "telegram"},
  "pii": {"handle": "@jdupont", "need": "fuite sous l'évier"}
}
```

`channel` other than `whatsapp` or `telegram` is rejected (`a bot source writes lead.created
only with channel whatsapp or telegram`). `messageRef` may be set when the bot made one.

### The customer wrote: `lead.messaged`

`properties.channel` is `whatsapp` or `telegram`. Name the lead by its id when the bot knows it
(it made the lead, or looked it up), or by the ref the customer brought (`/start AQ-7K3F`, the
`Réf. AQ-7K3F` line of the prefilled WhatsApp message) with no `leadId`:

```json
{
  "id": "0192f1c2-8a40-7c11-8e2b-0f6a5d4c3b2a",
  "schema": "sa.funnel.v1",
  "type": "lead.messaged",
  "typeVersion": 1,
  "occurredAt": "2026-10-07T09:14:30Z",
  "source": {"kind": "bot", "id": "aquafix-tg"},
  "subject": {"brandId": "aquafix"},
  "properties": {"channel": "telegram", "messageRef": "AQ-7K3F"},
  "pii": {"handle": "@jdupont"}
}
```

A ref is `^[A-Z]{2,4}-[0-9A-HJKMNP-TV-Z]{4,8}$` (the brand's prefix, Crockford base32 without I,
L, O, U). Refs are made by the landing's form and may repeat: the panel journals the event under
the brand's **newest** lead carrying it at that moment, and keeps that lead id — a later lead
with the same ref does not take the message over. A ref no lead of the brand carries yet is
`deferred` with a reason starting `unknown_ref`, and the batch is answered `409` with
`Retry-After`: the landing's `lead.created` may still be on its way (the site posts it in the
background as the customer taps). Send the **same** event again after `Retry-After`; nothing of
it was journaled. Give up after a few tries — a ref nobody's lead carries stays deferred. A
message resolved by its ref is journaled at the lead's location too, when it named none. Neither
`leadId` nor `messageRef`: `rejected`. A `leadId` the panel has no lead for is `deferred` the
same way (reason starting `unknown_lead`): send the bot's own `lead.created` first, or again.

The panel keeps the first message's time and messenger on the lead (`messaged_at`,
`messaged_channel`), and tells PostHog of that first one only (`sa_lead_messaged`); it does not
move the lead's stage — that is the operator's `lead.contacted`.

## Looking a lead up by its ref

```text
GET /api/ingest/v1/leads/by-ref/{brand}/{ref}          signed over "GET <path>"; a bot's key of {brand}

200 {"lead_id": "L-2", "channel": "telegram", "stage": "created",
     "created_at": "2026-10-07T09:12:00Z", "message_ref": "AQ-7K3F",
     "need": "fuite sous l'évier", "locality": "Royat",            what the customer asked for
     "quoted_cents": 9900, "flow": "fixed",                         when the landing showed a price
     "messaged_at": "…", "messaged_channel": "telegram"}            once they wrote
     — the optional fields absent when there is nothing; never a name or a phone
404 {"error": "no lead of the brand carries that ref"}
403 {"error": "only a bot's key looks a lead up by its ref"} | {"error": "this key is not for that brand"}
400 a brand or a ref that cannot be one (refs are upper case, as the landing made them)
```

The newest lead of the brand with that ref, as for `lead.messaged`. `need` is the customer's
words: show it back only in the conversation that brought the ref.

## curl

```sh
KEY=aquafix-tg SECRET=… PANEL=<the conductor's in-cluster URL>
sign() { printf 'sa-ingest/v1.%s.%s' "$1" "$2" | openssl dgst -sha256 -hmac "$SECRET" -r | cut -d' ' -f1; }

# lookup
TS=$(date +%s)
LOOKUP=/api/ingest/v1/leads/by-ref/aquafix/AQ-7K3F
curl -sS "$PANEL$LOOKUP" \
  -H "x-sa-key-id: $KEY" -H "x-sa-timestamp: $TS" -H "x-sa-signature: $(sign "$TS" "GET $LOOKUP")"

# the customer wrote
BODY=$(jq -cn --arg id "$(uuidgen-v7)" --arg at "$(date -u +%FT%TZ)" '{events: [{id: $id,
  schema: "sa.funnel.v1", type: "lead.messaged", typeVersion: 1, occurredAt: $at,
  source: {kind: "bot", id: "aquafix-tg"}, subject: {brandId: "aquafix"},
  properties: {channel: "telegram", messageRef: "AQ-7K3F"}}]}')
TS=$(date +%s)
curl -sS "$PANEL/api/ingest/v1/events" -H 'content-type: application/json' \
  -H "x-sa-key-id: $KEY" -H "x-sa-timestamp: $TS" -H "x-sa-signature: $(sign "$TS" "$BODY")" \
  --data-raw "$BODY"
```

`uuidgen-v7` stands for anything that prints a UUIDv7 (`uuid` ≥ 10 in Node, `uuid7` in Python);
the panel refuses other versions.

## Node (≥ 20)

```js
import { createHmac } from "node:crypto";
import { v7 as uuidv7 } from "uuid";

const { PANEL, KEY, SECRET } = process.env;

async function call(method, path, body = "") {
  const ts = String(Math.floor(Date.now() / 1000));
  // A GET has no body: the method and the path (with its query) are signed in its place.
  const signed = method === "GET" ? `GET ${path}` : body;
  const sig = createHmac("sha256", SECRET).update(`sa-ingest/v1.${ts}.${signed}`).digest("hex");
  const res = await fetch(PANEL + path, {
    method,
    headers: { "content-type": "application/json", "x-sa-key-id": KEY, "x-sa-timestamp": ts, "x-sa-signature": sig },
    body: method === "GET" ? undefined : body,
  });
  return { status: res.status, body: await res.json() };
}

// /start AQ-7K3F → show the summary, then say the customer wrote
const ref = "AQ-7K3F";
const lead = await call("GET", `/api/ingest/v1/leads/by-ref/aquafix/${ref}`);
const event = {
  id: uuidv7(), schema: "sa.funnel.v1", type: "lead.messaged", typeVersion: 1,
  occurredAt: new Date().toISOString(), source: { kind: "bot", id: KEY },
  subject: lead.status === 200 ? { brandId: "aquafix", leadId: lead.body.lead_id } : { brandId: "aquafix" },
  properties: { channel: "telegram", messageRef: ref },
};
const sent = await call("POST", "/api/ingest/v1/events", JSON.stringify({ events: [event] }));
// sent.status 207: results[0].status accepted | duplicate | rejected
// sent.status 409: results[0].status deferred (reason "unknown_ref…" / "unknown_lead…"): the same event again after Retry-After
```
