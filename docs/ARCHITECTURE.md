# Architecture

sites_panel is the Service-Arb sites business — the leads journal and its projections, places,
pricing, experiments, booking, invoices — behind the conductor (site_conductor). The conductor
signs people in, holds the source keys and the Telegram bot, and calls this service with an
assertion it signs for each request; sites_panel trusts nothing else. § numbers in the code refer
to `SA-PANEL-SPEC.md`.

```text
browser ─session─▶ conductor ──User assertion──────────▶ /api/v1/*             (its /api/sites/*)
landings, bots ─HMAC─▶ conductor ──Source assertion + body─▶ /api/ingest/v1/*
conductor's hubs ──Conductor assertion─────────────────▶ /api/conductor/*      (live, Telegram)
landings ─────────────────────────────── no assertion ─▶ /api/internal/*
providers ────────────────────────────── no assertion ─▶ /api/hooks/booking/*
browser ─────────────────── through the conductor ─────▶ /mfe/*                (its /sites/mfe/*)
```

A cargo workspace of four crates; dependencies point inwards only.

```text
contracts/proto/sa/v1/events.proto   sa.funnel.v1: the Event envelope, the ingest request and
                                     answer, one properties message per type@version
contracts/proto/scope/v1/            banking's ScopeLedger, vendored: the invoices
crates/panel_contracts/              those protos, generated at build time: prost messages + pbjson
                                     serde (protojson); tonic clients for the ledger
crates/panel_core/                   no I/O: no database, network, clock or randomness
  src/ids.rs                         EventId (UUIDv7, ev_lib's Id), brand/location/lead/job ids
  src/event.rs                       Envelope, TypeKey (type@version), KeyGrant: which kind and
                                     brands a key may write
  src/fact.rs                        the registered types, typed (Fact), and what each needs of
                                     its subject
  src/lead.rs                        Stage, and fold: a lead's facts → its stage and stage times
  src/notify.rs                      the notes: the texts and their code entities, the buttons
                                     (the rules and the wire shape are `sa_alerts`')
  src/experiment.rs                  a brand's experiments as configuration: a declaration
                                     checked, an admin's patch, the fold to declared / override /
                                     effective, the landings' rules for an override field
  src/analytics.rs                   what PostHog is told of a lead event (name, properties
                                     without PII, the person)
  src/place.rs                       a place's live settings (kitstart's PlaceLive), checked
                                     field by field; a change and who made it
  src/pricing.rs                     a brand's price list (kitstart's PricingModel): checked,
                                     priced to the cent; tests/fixtures/pricing vendored by sha
  src/booking.rs                     providers, a place's booking config and its URL rule, a
                                     lead's booking folded; tests/fixtures/booking vendored by sha
  src/phone.rs                       kitstart's normalizePhone, ported (E.164)
crates/panel/                        the engine
  src/lib.rs                         the `Panel` facade: ingest under a grant, PII, rebuild
  src/wire.rs                        protojson → the core: one event decoded and checked; the
                                     registry (type@version → properties message → Fact)
  src/seal.rs                        XChaCha20-Poly1305 at rest: PII, the notes
  src/store/                         SQLite (runtime sqlx queries, embedded migrations/,
                                     applied on open; writes BEGIN IMMEDIATE)
  src/store/events.rs                the journal
  src/store/projections.rs           leads, calls, payments
  src/notifications.rs               the fan-out into notes, and the notes read back
  src/store/notifications.rs         its candidates, claims (`fanout`) and `notifications`
  src/operator.rs                    what a signed-in user does and reads, as journaled events
  src/experiment.rs                  the experiments listed, changed by an admin (journaled), and
                                     what a landing is answered; the link to PostHog's funnel
  src/capture.rs                     the PostHog outbox sent in batches, retried, given up; the
                                     Capturer port
  src/place.rs                       a place's settings changed (optimistic concurrency, revert,
                                     withdraw) and what a site is answered
  src/pricing.rs                     a brand's pricing saved or removed (optimistic concurrency),
                                     the preview, and what a site is answered
  src/booking.rs                     operators' slots, the providers' seam (PushSource,
                                     PullSource), matching, the leased pull
  src/invoices.rs                    invoices issued through banking, mirrored, confirmed
  src/live.rs                        the in-process bus of what changed, published after each
                                     commit (see "The change feed")
  src/testing.rs                     (feature `testing`) throwaway SQLite files, batches, the
                                     conductor's assertions under a test key
  migrations/                        the schema: the init (`reporting_*` views and the
                                     journal's append-only triggers included), then one file
                                     per change, each with its `down`
crates/panel_server/                 the `sites_panel` binary: CLI and HTTP, thin over `Panel`
  src/http.rs                        the router: what each path takes, its bounds; the /api/v1
                                     route table and the section each sits under
  src/gate.rs                        the conductor's assertion, verified, per kind of route
  src/api.rs                         the operator API: JSON over the engine's `operator` module
  src/conductor.rs                   /api/conductor/*: the change feed, the notes, the receipts;
                                     the fan-out's schedule
  src/places.rs                      a place's settings: the editor's routes, and the sites'
                                     GET /api/internal/…
  src/pricing.rs                     a brand's pricing: the editor's routes, the preview, and
                                     the sites' GET /api/internal/brands/{brand}/pricing
  src/experiments.rs                 /api/v1/experiments and the sites' GET
                                     /api/internal/brands/{brand}/experiments
  src/capture.rs                     PostHog's capture API (reqwest) and the sender in `serve`
  src/booking.rs                     /api/v1 booking routes, POST /api/hooks/booking/…
  src/google_calendar.rs             the google_calendar pull adapter (reqwest), its schedule,
                                     the CLI's OAuth consent
  src/invoices.rs, scope_ledger.rs   /api/v1/invoices; banking's ScopeLedger over mTLS
  src/settings.rs                    the environment (ev_lib `settings!`)
  src/testing.rs                     (feature `testing`) the router called as the conductor calls it
crates/panel_gen/                    `nix run .#gen`: the front end's mirror of Rust types and
                                     tables (ev_lib `ts_gen`), into frontend/**/generated.ts,
                                     committed; the pre-commit hook re-runs it
sa_auth (the conductor's crate)      the `sa` permissions and the assertion: verified here
frontend/                            the `mfe-sites` bundle: <mfe-sites-panel> (the page, routes
                                     under `base`) and <mfe-sites-nav> (the conductor's Sites
                                     dropdown); `npm run build` → dist/ (PANEL_MFE_DIR); the
                                     element API (attributes, window events, routes) in its README
  src/app/                           defineElement, the /me gate, the page's routes (a lazy chunk
                                     per screen)
  src/shared/lib/router.ts           one location for both elements under `base`
  src/shared/lib/live/               the host's relayed socket (sa:live, sa:live-status)
```

## Ingest → journal → projections

```text
conductor: headers, the window, the key, its HMAC over the body (docs/BOT-API.md)   else 401 there
POST /api/ingest/v1/events        at most 32 at once (else 503), 30 s in all (else 408)
  │ an x-sa-assertion                                                       else 401, body unread
  │ body read: ≤ 4 MiB within 10 s                                          else 413 / 408
  │ the assertion: the conductor's, for this method and path, a Source over this body's digest,
  │ a kind a key is issued for, brands that are brand ids → KeyGrant      else 401
  │ body → {"events": [1..500]}                                             else 400
  ▼
each event on its own ─ decode (protojson) ─ envelope checks ─ key may write this kind and brand?
  │                                                                        no → rejected{reason}
  ▼
registry: type@version known?
  ├─ no  → journal, status unregistered                                   → accepted (not projected)
  ├─ yes, properties or subject wrong                                     → rejected{reason}
  └─ yes → one transaction:
             INSERT events … ON CONFLICT (id) DO NOTHING
               ├─ id taken, same content                                  → duplicate
               ├─ id taken, other content                                 → rejected
               └─ new: a call / payment row; its lead or its brand's experiments
                  recomputed; a PostHog outbox row for a lead event         → accepted
```

The conductor passes the answer back as it is. A bot (source kind `bot`) also looks a lead up
by its messenger ref: `GET /api/ingest/v1/leads/by-ref/{brand}/{ref}`, its assertion over an
empty body (the conductor's MAC covered `GET <path?query>`) — see [BOT-API.md](BOT-API.md) and
[Messenger leads](#messenger-leads).

## The conductor's assertion

`x-sa-assertion` (`sa_auth`): Ed25519 under the conductor's key (`PANEL_ASSERTION_KEYS`, several
while one rotates), `aud` sites, the method and the path as asked (no query), alive 60 s, and a
principal:

```text
User{sub, email, name, permissions}   /api/v1/*        permissions: the caller's sa:sites:* only;
                                                        sub a UUID, the Actor of what they write
Source{key_id, kind, brands, digest}  /api/ingest/v1/* the body's SHA-256; the KeyGrant
Conductor                             /api/conductor/*
```

Anything else on those routes — none, another key, another service, request or body, an
expired one, another principal — is `401 {"error": "unauthenticated"}`, its reason logged.
`/api/internal/*`, `/api/hooks/*`, `/mfe/*` and `/health` take none. Each `/api/v1` route sits
under a section — Work (`sa:sites:work:read`), Analysis (`sa:sites:analysis:read`) — or none
(`/me`); outside it, `403`. Actions ask their own permission on top (`sa:sites:work:leads:edit`,
`sa:sites:work:pii:see`, `sa:sites:work:places:edit`, `sa:sites:work:pricing:edit`,
`sa:sites:work:invoices:create`, `sa:sites:analysis:experiments:edit`). How fresh a permission
is, CSRF, and the session are the conductor's. `http::api_routes` is the table the router is
built from and the tests walk.

## The operator API, `/api/v1`

JSON; timestamps RFC 3339, money in minor units (|amount| ≤ 10^10, currency one of EUR,
USD, GBP, AUD — checked here, not in the registry, so a rebuild never re-judges old events),
errors `{"error": "…"}` with 400 / 403 / 404 / 409 / 503. Writes journal an event of kind
`panel` (manual), so they go through the same registry and projections as ingest.

`POST /leads`, `…/stage` and `…/payments` take an optional `Idempotency-Key` header (1–128
visible ASCII), `…/no_answer` a required one. The event id is then derived from (user, action, lead, key) — shaped as a
UUIDv7, its time field hash — and so is a new lead's id: a retry finds the first attempt in
the journal and is answered `200` with the same body, journaling nothing. Without the key,
every request is a new event (`201`).

```text
GET    /me                                        {sub, permissions}: the assertion's
GET    /leads?stage&brand&location&overdue&suspect&flow&channel&message_ref&created_from&created_to&cursor&limit
                                                  {leads: [Lead], next_cursor}; newest created
                                                  first, limit ≤ 200 (default 50); created_*
                                                  UTC days, both included; suspect=only |
                                                  exclude (absent: every lead); flow=quote |
                                                  estimate | fixed, anything else 400 (absent:
                                                  every lead; a lead that said no flow is under
                                                  none of the three); channel=form | phone_inbound
                                                  | callback | whatsapp | telegram; message_ref=
                                                  a ref as pasted (Réf. aq 7k3f: label, case,
                                                  spaces; O→0, I/L→1 in the code), the leads
                                                  carrying it
GET    /leads/counts?brand&location               {stages: {created: n, …, lost: n} (every
                                                  stage, 0 included), overdue, total}
POST   /leads                                     {brand, location, need, phone?, channel?} → 201
                                                  {brand, lead_id: "p-<uuidv7>", event_id};
                                                  channel phone_inbound (default) | whatsapp |
                                                  telegram, anything else 400
GET    /leads/{brand}/{lead}                      {lead: Lead, events: [Event]}
POST   /leads/{brand}/{lead}/stage                {stage: contacted, channel?} | {stage: quoted,
                                                  amount?, currency?} | {stage: won, job_id?} |
                                                  {stage: lost, reason, note?} | {stage: completed}
                                                  → 201 {event_id}
POST   /leads/{brand}/{lead}/messaged             {channel: whatsapp|telegram} → 201 {event_id}:
                                                  the customer wrote (lead.messaged); idempotent
POST   /leads/{brand}/{lead}/no_answer            → 201 {event_id}: call.attempted + call.logged
                                                  {no_answer} (a note's button)
POST   /leads/{brand}/{lead}/calls/attempt        → 201 {attempt_id}
POST   /leads/{brand}/{lead}/calls/{attempt}/outcome
                                                  {outcome: answered|no_answer|wrong_number|later}
POST   /leads/{brand}/{lead}/payments             {billed, commission, currency}
GET    /funnel?from&to&brand&by                   days, UTC, default the last 30, ≤ 366
                                                  {from, to, brand, min_sample, posthog_url,
                                                   stages: [{stage,
                                                   reached, of_previous, of_leads}], lost, manual,
                                                   payments: [Paid]}
                                                  by=location: {from, to, brand, min_sample,
                                                   by, locations: [{brand, location, stages, lost,
                                                   manual, payments}]}, one per brand's location
                                                   (location null for the leads naming none);
                                                   an empty window has no rows
GET    /experiments?brand                         see [Experiments](#experiments)
GET    /places                                    {places: [{brand, location, last_lead_at,
                                                  has_settings, withdrawn}]}: every location a
                                                  lead names, and every place registered
                                                  (below)
```

The funnel counts the leads that came in (were created) within the window, and `Paid` —
`{currency, billed, commission, count}` in minor units, one per currency, never converted —
sums the payments of those same leads, whenever they were paid; so a slice's money and its
`paid` step are about the same leads.

**Stages 3–4 (visits, intents) and the experiments' numbers are PostHog's**, looked at there:
the panel does not count them again (owner, 2026-10-04). It sends PostHog the leads' life after
the form instead ([PostHog](#posthog)), so the funnel from a visit to a payment is one funnel
there.
`posthog_url` opens it (`PosthogProject::funnel_url`): `location_page_view` → `sa_lead_created` →
`sa_lead_contacted` → `sa_job_won` → `sa_payment_received` over the same days, within 90 days of
the visit, filtered by `brand_id`, or broken down by it without `brand`. Null without
`POSTHOG_PROJECT_ID`.

`Lead` is the projection row (`stage`, the time of each stage, `manual`, `lost_reason`,
`suspect` — null, `"rate_limited"` or `"too_fast"`, see [Suspect leads](#suspect-leads) —,
`flow`, `quoted_cents`, `pricing_valid_from`, `estimate_inputs` — see [Flows and
prices](#flows-and-prices) —, …)
plus `sla` while it waits for its first contact — `{waiting_since, waiting_seconds,
overdue}`, overdue after 30 minutes — and `pii` (the customer's name, phone, need) for a caller
holding `sa:sites:work:pii:see`. A share is `{n, of, percent, small_sample}`; `percent` is null while `of`
is under `min_sample` (§10.1), so the front end can only draw "n of of".

## Suspect leads

A landing's antispam sorts what its form receives three ways. A submission the honeypot
caught is a bot: the landing drops it and the panel never hears of it. One that is plausible
but doubtful is sent as an ordinary `lead.created` with `properties.suspect` set — it may be a
person, so it is kept rather than lost:

```text
suspect absent    an ordinary lead
"rate_limited"    the visitor's address sent more than the landing allows in its window
"too_fast"        the form came back sooner after it was shown than a person types
anything else     the event is rejected, like any word outside a closed vocabulary
```

The field is an extension of `LeadCreatedV1` (optional, `type_version` stays 1). A build
before it knows no such field and rejects an event carrying one, so the landings send it only
once the panel that takes it is live. The mark is the counted creation's (the first
journaled), so a later clean `lead.created` does not clear it, and progress does not either:
a suspect lead that is called and won is still one the antispam doubted.

Where it shows: `leads.suspect` (`TEXT`, NULL or one of the two words by CHECK), recomputed
like every column of the row, so the rebuild restores it from the journal; `suspect` on
`Lead` in `/api/v1` and the `suspect=only|exclude` filter of `GET /leads`; a `changed{topic:
leads}` like any new lead. `reporting_leads` carries the column; `reporting_funnel_daily`
still counts a suspect lead in `leads` and every stage it reaches, and counts it apart in
`suspect`, so a report that wants them out subtracts. A note tells of one under `new_lead`,
headed as suspect with its reason and with the usual buttons, and none reminds of it past the
contact SLA: nobody promised to call it back within 30 minutes.

## Flows and prices

A landing offers each need through one of three flows (FORM-VARIANTS-SPEC, lib#178): `quote`
(the customer asks for a price), `estimate` (a price computed from enum choices the visitor
made — zone, bedrooms, frequency, …) or `fixed` (a fixed price for a well-defined job). The
landing's server computes the price itself and says, in `lead.created`'s properties, what it
showed:

```text
flow                quote | estimate | fixed; absent: a landing from before the flows
quoted_cents        int64 ≥ 0, integer cents EUR TTC (no currency field yet)       ┐ both, exactly when
pricing_valid_from  RFC 3339 full-date, the day the pricing model took effect     ┘ flow is estimate|fixed
estimate_inputs     {input id: value id}, slugs of 1–40 [a-z0-9_-], ≤ 12 pairs; only with estimate
```

Anything else is rejected like any word outside a closed vocabulary: a price with `quote` or
no flow, a priced flow without both fields, one of the two alone, inputs with another flow, a
key or value not a slug (never free text: they sit in the clear), 13 pairs, a date that is
not a real day. The fields extend `LeadCreatedV1` (optional, `type_version` stays 1); a build
before them rejects an event carrying one, so kitstart sends them only once this panel is live
(its `panelFlow` switch).

Like `suspect`, they are the counted creation's (the first journaled): a later `lead.created`
saying nothing does not clear them. Where they show: `leads.flow`, `quoted_cents`,
`pricing_valid_from` (`TEXT`, a real day by CHECK) and `estimate_inputs` (JSON object), NULL
when the lead said nothing, rebuilt from the journal like every column; the same four on `Lead`
in `/api/v1` (null when absent) and the `flow=` filter of `GET /leads`. `reporting_leads`
carries the four; `reporting_funnel_daily` counts the `estimate` and `fixed` leads apart.

## Messenger leads

MESSENGER-CHANNELS-SPEC §3. A landing's messenger variants let the customer write on WhatsApp
(our prefilled message, ending `Réf. AQ-7K3F`) or open the brand's Telegram bot
(`t.me/<bot>?start=AQ-7K3F`) instead of leaving a phone number; a bot or an auto-responder may
start a lead from a conversation of its own.

```text
lead.created     channel += whatsapp | telegram; message_ref (optional): the ref the landing made,
                 ^[A-Z]{2,4}-[0-9A-HJKMNP-TV-Z]{4,8}$, not PII, not unique
lead.messaged@1  {channel: whatsapp|telegram, message_ref?}: the customer actually wrote. Writers
                 bot | panel (a landing only knows a link was opened). Subject: the lead, or
                 none and a ref → the brand's newest lead carrying it, looked up under the
                 journal's write lock and journaled as the event's lead_id (so a rebuild, when a
                 newer lead may carry the ref, does not look again; the content MAC is of the
                 event as sent, so a resend is a duplicate); no such lead yet → deferred
                 "unknown_ref: …" (409 + Retry-After, not journaled; the bot sends it again);
                 resolved by ref, it takes the lead's location when it named none. A leadId
                 with no lead yet: deferred "unknown_lead: …" the same way, never a phantom lead
source kind bot  a key per bot (issued at the conductor): lead.created with a messenger's
                 channel only, lead.messaged, and the lookup by ref (the only kind that may)
```

On the lead: `leads.message_ref` (the counted creation's, indexed with the brand),
`messaged_at` and `messaged_channel` (the first message, both or neither); a message moves no
stage. `/api/v1` `Lead` carries `channel`, `message_ref`, `messaged_at`, `messaged_channel`;
`GET /leads` filters on `channel=` and `message_ref=`; an operator takes a messenger lead in by
hand (`POST /leads {channel}`) and says a customer wrote (`POST …/messaged`).
`reporting_leads` carries the three columns, `reporting_funnel_daily` counts `messaged`.
PostHog is told `sa_lead_messaged {channel}` once per lead, for the first message the panel
journaled, never the ref. An operator's `POST …/messaged` on a lead that has a message already
journals nothing and answers `200` with that message's id.

The place's `telegram` (its bot's username) and `messengers` (kill switches) are place
settings, below.

## The conductor's feeds, `/api/conductor`

### The change feed

The conductor's one live socket per screen is told when what it read may have changed; the hint
comes from here, the data always from `/api/v1` under its checks.

```text
GET /api/conductor/changes     application/x-ndjson, open until the process stops
  {"type":"resync"}            first on every connection, after a rebuild in the serving
                               process, and when the reader fell 1024 behind
  {"type":"changed","topic","brand_id"?,"id"?,"at","permission"}   after each commit
  {"type":"ping"}              every 25 s: a quiet feed is told from a dead one
```

| `topic` | when | `brand_id` | `id` | `permission` |
| --- | --- | --- | --- | --- |
| `leads` | a lead came in (`lead.created`: ingest, or typed in) | its brand | the lead | `sa:sites:work:read` |
| `lead` | a stage, a call, a payment of one lead (the API or a Telegram button) | its brand | the lead | `sa:sites:work:read` |
| `places` | a place's settings set, reverted, withdrawn, restored, or the place registered | its brand | the slug | `sa:sites:work:read` |
| `pricing` | a brand's pricing saved or removed, or its locales set | its brand | — | `sa:sites:work:read` |
| `experiments` | a landing's declaration, or an admin's change | its brand | the key (a change) | `sa:sites:analysis:read` |
| `bookings` | a provider's booking without a lead came, changed, or was attached | its brand | — | `sa:sites:work:read` |
| `invoices` | an invoice stored, read from banking (changed), dropped on a refusal, or paid / cancelled / expired by the confirmation | — | the invoice (one row); — (a confirmation page) | `sa:sites:work:read` |

A booking event tells `lead` for every lead it changed — a provider's booking attached
elsewhere tells the lead it left and the one it joined. `at` is when the write committed.

- **`permission` is the read's.** `panel::live::Change::permission` is the section that reads
  the topic, for every brand; the conductor tells a socket whose caller holds it.
- **Published after the commit, from the engine, in one place per kind of write.** Every
  event that reaches the projections passes through `Panel::journal`, which publishes once
  its transaction has committed — ingest, the operator API and an admin's experiment changes
  alike, so none of them can forget to. A duplicate, a refused event or an unregistered type
  changed no read and says nothing. Outside the journal: a place's change (`place.rs`, after
  its transaction), a brand's pricing (`pricing.rs`, the same), the invoices (`invoices.rs`,
  after each commit that changed a row). A write that changes nothing publishes nothing.
- **The bus is in-process** (`tokio::sync::broadcast`, one pod). Publishing never waits and
  never fails a write. A command run in another process (`sites_panel place set`) is not on the
  feed: the screens find it at their next read.
- **Shutdown** ends the feed first (`Signal::GoingAway`): an open response would hold the
  graceful shutdown forever.
- **Not told:** time passing. A lead becomes overdue (`sla`) with no write; the screens keep
  their own clock for it.

### Notifications

What the conductor's alerts tell of the sites. The fan-out here finds what is due; who gets
it, the rules' toggles, delivery, pacing, retries and the buttons' callback data are the
conductor's.

```text
fan-out (2 s)    new leads (their counted creation ≤ 1 h old, still `created`; a suspect one
                 headed "Suspect lead (antispam: …)"), leads created 30 min – 6.5 h ago never
                 contacted (suspect ones not), payments (≤ 24 h), bookings made, moved or
                 canceled (≤ 6 h; "… без заявки" for one without a lead)
                 → `fanout` claims (rule, event) once and, in the same transaction, appends
                 one note to `notifications`
GET /api/conductor/notifications?after=<seq>&limit=<1..500>
                 `sa_alerts::NotePage` {notes: [{seq, rule, event_id, created_at, text,
                  text_pii | null, skip_user | null, buttons}], next}: the notes after `after`,
                  oldest first, each passing `Note::check` against sites' feed
```

- **`seq`** is AUTOINCREMENT under the one writer, so it grows in commit order and is never
  handed out twice: a cursor never skips a note. Notes are kept 24 h, claims 7 days.
- **Texts** are Telegram's: `{text, entities: [{type: "code", offset, length}]}`, offsets in
  UTF-16 units, sent with no parse mode. `text` carries no PII; `text_pii` (brand, location,
  name, need, phone) is there when the lead left any, for the conductor to send only to holders
  of `sa:sites:work:pii:see`. What a customer typed is put on one line (control characters,
  line separators and bidi marks become spaces), bounded (name 80, need 500, phone 40 of
  digits and `+()-`, the message 3 500), and the name and need are `code` entities — so nothing
  is markup, no link is made of it, and no line of it can read as one of ours ("Взял: …").
  Both are sealed under `PANEL_DATA_KEY`, the rule and the event bound.
- **`skip_user`**: whoever typed the lead in, or set or closed the slot, knows of it already.
- **Buttons** (lead notes): `{id, label, pressed, call: {method, path, body}, leaves}`. "Взял"
  is `POST /api/v1/leads/{brand}/{lead}/stage {stage: contacted}`, "Не дозвонился" `POST
  …/no_answer`; the conductor makes the call as the user who pressed, with `Idempotency-Key:
  tg/<its outbox id>/<button id>`, so a press repeated records nothing twice. Labels in
  `TELEGRAM_LOCALE` (`ru`, `en`).

### Each key's last receipt

`GET /api/conductor/internal-reads` → `{since: <boot, RFC 3339>, reads: {<brand>: {locations?, pricing?,
experiments?: <RFC 3339>}}}`: the last `/api/internal` read per brand and endpoint, kept in memory
only (`conductor::InternalReads`, nothing written on the read path, so it restarts empty; `since`
says from when). At most 256 brands: past that a new brand is not recorded and none is evicted, so
an unauthenticated caller cannot push the real ones out. The router mounts `/api/internal` from
the same constant `--print-provided-paths` prints, which `checks.contract-provides` holds to the
contract's `provides.internal`.

`GET /api/conductor/sources/received` → `{<key_id>: <RFC 3339>}`: when each key the journal
names last had an event received (`max(events.received_at)`); the conductor's "source silent".

## Place settings

A landing bakes its places into its build and lays over each, field by field, what the
panel answers for it (kitstart's `createPlaceSource`, `PlaceLive`): `phone`, `whatsapp`
(E.164), `telegram` (the place's bot, its username without the `@`:
`^[A-Za-z][A-Za-z0-9_]{1,28}[Bb][Oo][Tt]$`, 5–32 characters ending in bot), `messengers` (`{whatsapp?: bool, telegram?: bool}`, the
landing's messenger buttons switched off; absent is on, another key refused as
`messengers.<key>`), `hours` (`[{days, opens, closes}]`), `serviceArea` (commune names), and for
storefronts `address`, `geo`, `storefrontPhoto` (https), `landmark` (a text per locale),
`rating`. Hours are the place's local time, Europe/Paris for every place in v1 (no
time zone per place). kitstart fetches every 10 minutes at most, times out after 3 s, and keeps its baked
place on a 5xx or no answer; a JSON 404 is a place withdrawn, and the site 404s it.

```text
GET  /api/internal/brands/{brand}/locations/{slug}?locale   no assertion; ≤ 32 at once, 3 s
       200 the settings, only the fields set; {} for none, for a place the panel does not
       know, and for a brand or slug that cannot name one — never a 404 for those, or an
       empty database would take the sites down
       404 {"error": "not_found"}: only a place an admin withdrew
```

Under `/api/v1` (Work); writes need `sa:sites:work:places:edit`; without it, a read (`can_edit`
false, `403` on a write):

```text
GET  /places/{brand}/{slug}/settings          {brand, slug, withdrawn, settings, updated_at,
                                              updated_by, can_edit}; updated_at null: never set
PUT  /places/{brand}/{slug}/settings          {settings, expected_updated_at} → 200 as GET; a
                                              full replace; 409 {"error": "conflict"} when
                                              expected_updated_at is not the current one;
                                              422 {"error": "invalid", "fields": {key: why}},
                                              a key the field or its path in a list:
                                              hours[0].opens, hours[1].days, serviceArea[2]
GET  /places/{brand}/{slug}/settings/history  {changes: [{id, at, by, kind, before, after,
                                              reverts}]}, newest first; kind register | set |
                                              revert | withdraw | restore
POST /places/{brand}/{slug}/settings/revert/{id}  the `before` of that change made current,
                                              journaled as a revert → 200 as GET; 404; body
                                              {expected_updated_at} optional, 409 as a PUT
POST /places/{brand}/{slug}/withdraw|restore  → 200 as GET; the settings are kept
POST /places                                  {brand, slug} → 201 as GET; 409 {"error":
                                              "exists"} once registered (by hand or an edit)
PUT  /places/{brand}/{slug}/gmb               {gmb_id: n | null} → 200 as GET (with gmb_id); 404
                                              unknown place; 409 {"error": "gmb_taken"}
GET  /gmbs                                    {"<gmb_id>": {brand, location}} for every
                                              place with a gmb_id (Work read)
```

- **A place says which GMB it is the landing of** (`places.gmb_id`, the conductor's `gmbs.id`,
  held as opaque; unique among places, nullable). Not journaled in `place_changes` (that holds
  settings); the change is published as `places`.

- **Checked as kitstart reads them, refused instead of dropped.** kitstart drops a field it
  cannot read so a page never fails on one; the panel names it in a 422 so the editor sees
  why. Stricter than kitstart where kitstart is loose (a phone is E.164, not just `+…`;
  hours close after they open, a night across midnight is two rows; a day once per row),
  never looser. kitstart keeps a landmark only when it has every locale the site speaks,
  which the panel does not know.
- **A change is one write transaction**: the current settings read under the write lock,
  `expected_updated_at` compared, the new ones written, the change journaled
  (`place_changes`: before, after, who — the user's email and id, or `cli` — and
  when). `updated_at` only grows, a microsecond at least per change, so two edits never share
  a token. A change that changes nothing writes nothing.
- **The CLI** (`sites_panel place set|show|history|revert|withdraw|restore`) goes through the same
  calls, `by = cli`, patching over what is there when its transaction begins.
- **History is append-only, places never deleted**: triggers refuse any UPDATE or DELETE of
  `place_changes`, a DELETE of `places` or `place_settings` (cleared is `{}`), and any UPDATE
  of `places` but `withdrawn` and `gmb_id`.

## Pricing

A landing prices its `estimate` and `fixed` needs by a brand's price list (kitstart's
`PricingModel`, FORM-VARIANTS-SPEC, lib#178), baked into its build and replaced by what the
panel answers for the brand (`createPricingSource`: every 10 minutes at most, 3 s, the baked
model on any failure or on a model it refuses). **The model is kitstart's**: its shape, its
rules and its arithmetic are normative in kitstart's fixture README,
`crates/panel_core/tests/fixtures/pricing/README.md` (format 1, EUR TTC, integer cents,
basis points, every intermediate product rounded half up to the cent, the total to
`roundToCents`, then `minimumCents`; a `fixed` need as is). `panel_core::pricing` is a port of
`validate.ts` and `price.ts`, held to the same fixtures.

- **Vendored by sha.** `crates/panel_core/tests/fixtures/pricing/` is a copy of kitstart's
  `ts/kitstart/test/fixtures/pricing` at the commit `SOURCE` names; `tests/pricing_fixtures.rs`
  accepts every `valid/*.json` (and reads it back unchanged), refuses every `invalid/*.json` at
  the field its name says, and prices every `cases.json` to the cent, `null` included. kitstart
  changing a fixture changes the contract: copy the directory of the new commit over this one,
  write its sha in `SOURCE`, make the tests pass (adding the line a new invalid fixture needs),
  and ship the panel before the sites rely on it.
- **A brand's locales.** Every input's and option's label must be in each locale the brand's
  sites speak, or a site refuses the whole model; the panel refuses to save one (kitstart's
  `pricingProblemsFor`). The locales are `fr,en` unless set by `sites_panel pricing locales <brand>
  fr,en` (`brand_locales`); a model saved before a locale was added is kept for the editor but
  not served (`{}`) until saved with the labels.
- **Paths.** A refusal names the first problem at kitstart's path without its `model.` root:
  `needs.standard.inputs[2]`, `inputs[0].options[1].labels`. JSON objects are read sorted here
  and in insertion order there, so with several problems the first named may differ; whether
  there is one never does. Saved, a model is stored in kitstart's shape (its maps' keys sorted).

```text
GET    /api/internal/brands/{brand}/pricing?locale   no assertion; with the locations' read ≤ 32
       at once (shed: `{}`), 2.5 s (then `{}`); `locale` ignored, every locale's labels sent
       200 the model; {} for none, for a brand id that cannot be one, for a model the brand's
       locales no longer pass, and on any failure of the store (logged) — never a 404 or 5xx
```

Under `/api/v1` (Work); saving and removing need `sa:sites:work:pricing:edit`; reading and the
preview are the section's:

```text
GET    /pricing                       {items: [Item]}: every brand the panel knows (a lead's,
                                      a place's, a landing's experiments') or has
                                      pricing or locales for
GET    /pricing/{brand}               Item; a brand never set: model, updated_at, updated_by null
PUT    /pricing/{brand}     edit      {model, expected_updated_at: RFC 3339 | null} → 200 Item;
                                      409 {error: "stale", current: Item}; 422 {error, path}
DELETE /pricing/{brand}     edit      {expected_updated_at} (required) → 200 Item, model null;
                                      409 as PUT
POST   /pricing/{brand}/preview       {model, need, inputs: {input: option}} → 200 {cents: n |
                                      null} (null: kitstart's "no price"); 422 as PUT: the draft
                                      is held to the brand's locales, as the site would
GET    /pricing/{brand}/changes       {changes: [{id, at, by, kind: set | remove, valid_from,
                                      needs, model}]}: the last 50, newest first; `model` the
                                      one the change left; a removal's three null

Item: {brand_id, locales, model | null, updated_at | null, updated_by | null}
```

- **A change is one write transaction**, as a place's: the current model and the brand's
  locales read under the write lock, `expected_updated_at` compared, the model checked and
  written, the change journaled (`pricing_changes`: before, after — NULL for none —, who, when).
  A removal keeps the row with `model` NULL and its own `updated_at`, which the next save names;
  `updated_at` only grows, a microsecond at least per change. Saving the model there is, or
  removing none, writes nothing and tells nobody.
- **The CLI**: `sites_panel pricing show|history|set <brand> <file|->|remove|locales`, `by = cli`,
  over whatever is there when its transaction begins.
- **Append-only**: triggers refuse any UPDATE or DELETE of `pricing_changes` and a DELETE of
  `pricing`.

## Experiments

A landing's experiments live in its code (`@evinvest/experiments`); the panel holds them as
configuration — what each brand runs, and an admin's kill switch, weights and holdout over it.
Exposures, leads and their comparison are PostHog's: each experiment links to its funnel there.

```text
experiments.declared@1   the landing at every start (kind site, its key; a new id each time),
                         subject the brand alone: [{key [a-z0-9_]{1,64}, variants 2–32 unique
                         [a-z0-9_-]{1,32} (the first the control), weights one per variant ≥ 0
                         summing > 0, enabled, holdout? [0, 1), summary? ≤ 200}], ≤ 64, a key once
experiment.configured@1  an admin in the panel (kind panel, source.id the admin's id or cli),
                         subject the brand alone: {key, enabled?, weights?, holdout?, reset: [field], by —
                         the admin's email, else their id, as a place's history names them} — a
                         patch: a field absent left, named in reset put back to the declaration
```

Both are folded per brand (`panel_core::experiment::fold`, by `(occurred_at, id)`, so arrival
order does not matter) into `experiments`, recomputed in the transaction that journals either and
by the rebuild. The latest declaration is the brand's: an experiment it does not name is
`retired` — listed, never changed, never sent to a site. An override is kept as set; a field of
it is applied only while valid against the latest declaration, by the landings' own rules
(`applyOverrides`): weights as many as the variants, each ≥ 0, sum > 0; holdout in [0, 1);
enabled a boolean. So `effective` is what the landings run even after a declaration changed the
variants under an admin's weights. `weights_changed_at` is the last time the effective weights
changed, by a declaration or an admin.

```text
GET /api/v1/experiments?brand            Analysis; one brand's, or every brand's without it
     {experiments: [{brand, key, variants,
       declared: {weights, enabled, holdout, summary, declared_at},
       override: null | {weights, enabled, holdout (each null: follows the declaration),
                         changed_by (the admin's email, else their id: by), changed_at},
       effective: {weights, enabled, holdout}, weights_changed_at, retired, posthog_url}]}
PUT /api/v1/experiments/{brand}/{key}    sa:sites:analysis:experiments:edit
     {enabled?: bool | null, weights?: [n] | null, holdout?: n | null}: absent left, null put
     back → 200 the item; 400 {error} invalid (weights against the declared variants); 404 an
     unknown or retired experiment, or a brand or key that cannot be one; a change that changes
     nothing journals nothing
GET /api/internal/brands/{brand}/experiments   no assertion, with the places' and pricing's reads
     ≤ 32 at once (shed: no overrides), 2.5 s (then none)
     200 {experiments: {key: {enabled?, weights?, holdout?}}}: the valid override fields of the
     current experiments; {experiments: {}} for none, an unknown brand, a brand id that cannot be
     one, any failure — never a 404 or 5xx
```

`posthog_url` is `{POSTHOG_APP_HOST}/project/{POSTHOG_PROJECT_ID}/insights/new#q=<query>`: an
`InsightVizNode` with a `FunnelsQuery` `experiment_exposed` → `experiment_lead`, event properties
`experiment = key`, `brand_id = brand`, `forced` not `true`, broken down by `variant`, from
`weights_changed_at` or the first declaration. Null without `POSTHOG_PROJECT_ID`.

## PostHog

The panel tells PostHog what happens to a lead after the form, so a visit and its payment are one
funnel there (`panel_core::analytics`):

```text
queued       Panel::journal, in the event's own transaction, when POSTHOG_PROJECT_API_KEY is
             set: a new lead.created (the one that counts), lead.messaged, lead.contacted, lead.quoted,
             job.won, lead.lost, job.completed, payment.received, call.logged, every booking.*
             → posthog_outbox
             never: the rebuild (it projects without passing there), a duplicate, a second
             lead.created, call.attempted, what names no lead (a provider's booking.created or
             booking.canceled no lead was matched to)
event        sa_ + the type, dots made underscores (sa_lead_created, sa_payment_received, …);
             booking.status_changed is sa_booking_closed
uuid         the journal's event id: PostHog deduplicates a resend
timestamp    occurred_at
distinct_id  the lead's analytics_id (lead.created's, the landing beacon's distinct_id), else
             sa-lead:<brand>:<lead>
properties   brand_id, location_id, manual; channel, flow, quoted_cents, suspect (created);
             channel (messaged, contacted); amount_cents, currency (quoted); reason (lost — the slug,
             never the note); billed_cents, commission_cents, currency (paid); outcome (call);
             provider, preferred_date, preferred_part (booking requested); provider, match,
             lead_time_hours (booking created: start_at − booked_at, else − occurred_at);
             provider (canceled); lead_time_hours (set); closed (closed); provider, match =
             manual (attached); nothing more (cleared).
             No PII: no need, name, phone, note, attendee; no external_ref, version or slot
sent (5 s)   ≤ 100 due rows → POST {POSTHOG_HOST}/batch/ → deleted; 5xx, 429, no answer: each
             row again from 10 s, doubling to 15 min; another 4xx: a batch is retried row by
             row, a row refused alone dropped (warned); a row failing for 7 days dropped (warned)
```

Without `POSTHOG_PROJECT_API_KEY` nothing is queued and nothing sent; `serve` warns and starts.
The key is the project's `phc_` key — public like the landings' — not a personal one (`phx_` is
refused at boot). A booking's slot is told as `lead_time_hours` alone: its weekday and part of
the day would need the place's time zone, which a fact does not carry.

## Invoices

An operator asks a customer to pay in USDT through banking: the panel issues an invoice in
banking's scope `sa` (`scope.v1.InvoiceService`, protos vendored in `contracts/proto/scope`)
and opens its pay page on evinvest.ltd for the customer. Banking is the authority; the
`invoices` table mirrors what it last said (`panel::invoices`, `panel_server::scope_ledger`).

```text
POST /api/v1/invoices      sa:sites:work:invoices:create, Idempotency-Key required
                           → the row is stored `creating` with the terms, then banking is asked
                           (OpenAccount `sa:invoices` once, CreateInvoice under a key derived from
                           the row, subject_ref sa_invoice:<row id>) → 201; a retry → 200, the
                           same row; the key with other terms → 409. A refusal drops the row;
                           a lost answer keeps it for the retry.
GET  /api/v1/invoices      the rows, newest first, ≤ 500, no call to banking
GET  /api/v1/invoices/{id} GetInvoice → stored; a `creating` row is asked for again; a final
                           one (paid, cancelled, expired) is answered as stored
confirm (15 s)             MoneyEvents.Pull after the stored cursor: INVOICE_PAID marks the
                           invoice paid once (the event id is stored on it), CANCELLED and
                           EXPIRED end it; the cursor moves in the same transaction
```

**Paying in the page.** Each open row mounts banking's `<mfe-banking-pay-invoice invoice
scope="sa" actions="inline">`. When banking's wallet BFF lists `pay` for this origin
(`WALLET_ACTION_ORIGINS` / `WALLET_ACTION_KINDS`, banking docs/WALLET-INTENTS.md) the element
shows the terms and asks the passkey in this page, then dispatches `ev:wallet-result`; else (or
before the list arrives, or without a passkey) it offers the popup on evinvest.ltd as before.
Either result is a hint: the row is read again from banking, and the confirmation's `invoices`
frame turns it paid. `wallet.js` is pinned (`WALLET_SCRIPT_INTEGRITY`, SRI on the script the
loader injects): banking's registry carries no integrity, so a banking release that changes
`wallet.js` needs a panel release with its new `mfe:pins` hash, or the wallet and Pay read
"unavailable". The front end loads no Sentry Replay or PostHog; adding either must keep
`/api/auth/stepup/*` and `/cabinet/api/wallet/intents*` out of their network-payload capture
(the step-up proof and the CSRF token ride there).

Paid means posted: `processing` (the payer paid, the ledger has not posted it) is never paid,
and a paid invoice is never anything else. Nothing here writes the journal or `payments`: a
lead's payments are typed in by hand and stay apart. Banking proves the panel twice, by its
client certificate (mTLS) and by a `typ=service` token minted over the same channel
(`ScopeAuthService.MintServiceToken`) and cached until shortly before it lapses. Without
`SCOPE_LEDGER_*` the routes answer 503 `invoices_unavailable`; a grant banking has not given
(`not_granted`) and an unreachable banking (`ledger_unavailable`) are 503 too.

## Booking

FORM-VARIANTS-SPEC, "Booking providers contract" (2026-10-04). Providers, a closed set:
`manual | link | google_calendar | cal_com` (`calendly` next, same seams). kitstart's booking
fixtures are vendored by sha in `crates/panel_core/tests/fixtures/booking` (`SOURCE`), their
README normative; `tests/booking_fixtures.rs` holds the config validator to `valid/` and
`invalid/`, `crates/panel/tests/booking.rs` the registry to `requested/`. `choose.json` and
`hrefs.json` are the site's (the panel neither picks a provider nor builds a link).

**A place's `booking`** (place settings, `PlaceLive.booking` in `/api/internal/…/locations`):
`{"default": <provider>, "providers": {<provider>: {"url"}}}`. `manual` is always available and
never a key of `providers`; `default` is `manual` or a key of it. URL rule (every page): ≤ 2048
printable ASCII, literal `https://`, no `#` nor `\`, no `@`/`:`/`[` in the authority, a dotted
DNS name whose last label is not a number; query allowed. `google_calendar`:
`calendar.app.google/…` or `calendar.google.com/calendar/appointments/…`; `cal_com`:
`cal.evinvest.ltd` or `cal.com`, path exactly `/<user>/<event>`; `link`: any host. A 422 names
`booking.default`, `booking.providers.<p>.url`, ….

**Journal types**

```text
booking.requested@1      site    {lead_ref = subject.lead_id (lead-<row>-<8hex>), provider,
                                 preferred_date?, preferred_part? (manual only)}; no null, no
                                 free text; the date 2 days back – 366 ahead of arrival (checked
                                 at ingest, not by the registry: a rebuild never re-judges it).
                                 Before its lead's lead.created: verdict `deferred`, the batch
                                 answered 409 + Retry-After 30 (kitstart retries 409/425 on it)
booking.created@1        booking {provider (with an adapter), external_ref, start_at, end_at?,
                                 match? (ref|contact, exactly with subject.lead_id), version,
                                 booked_at?}; the attendee in pii, sealed
booking.canceled@1       booking {provider, external_ref, version}
booking.set@1            panel   {start_at, end_at?}             → booked
booking.status_changed@1 panel   {status: done|no_show|canceled} → only from booked
booking.cleared@1        panel   {}                              → none
booking.attached@1       panel   {provider, external_ref}, subject.lead_id the lead
```

`source.kind = booking` is one kind for every adapter (the provider in the properties); no key
is ever issued for it (nor for `panel`). Migration `20261005090000_booking_source_kind` widened
the journal's CHECK by making `events` again: `-- no-transaction`, `PRAGMA foreign_keys = OFF`
outside a `BEGIN IMMEDIATE` copy, every row, index, trigger and `reporting_ingest_daily` made
again; its test checks the copy byte for byte, `foreign_key_check`, the triggers, and the way
down (refused while a `booking` event exists).

**Projections.** `booking_events` (a row per booking event), `bookings` (a provider's booking:
`(brand, provider, external_ref)`, its id derived from them; its slot, booked or canceled, its
lead and `match` — NULL while unmatched), and a lead's `booking_*` columns, folded
(`panel_core::booking::fold`) from its own booking facts and the events of the providers'
bookings joined to it now. Statuses `none | requested | booked | canceled | done | no_show`;
an operator's action not allowed from where it stands is a 409, and the fold passes over it
too, so a race lands where the first left it. A provider's change does not reopen a booking
an operator closed. A provider's booking is joined by: the latest `booking.attached`, else the
match its first event that named a lead was journaled with.

**The seam** (`panel::booking`). Push: `PushSource::verify(PushRequest{brand, headers, body,
now}) → Vec<BookingEvent>`, routed at `POST /api/hooks/booking/{provider}/{brand}` (no
assertion; 256 KiB, 8 at once, 10 s; 401 refused, 400 unreadable, 200 `{written, duplicate,
unchanged, ignored, refused}`); **no push provider is registered** — every provider answers 404
there. Pull: `PullSource::pull(brand, cursor, now) → {events, cursor}`, run by
`Panel::sync_bookings` under a lease per (provider, brand) in `booking_sync` (5 min; retried
after 1 min on failure); the cursor moves only once what it covers is journaled; a cursor the
provider dropped (`CursorExpired`) is a full pull. `Panel::ingest_bookings` journals each
`BookingEvent` with an id derived from (provider, brand, ref, type, revision): a revision seen
again is a duplicate; a slot as it stands already is `unchanged`; a cancellation of a booking
the panel never had is ignored.

**Matching**, once per booking (later events keep its lead): a `lead_ref` naming a lead of the
brand → `ref`; else the attendee's phone (E.164, kitstart's rule) or email (lowercase) against
the `phone` / `email` of the creation PII of the brand's leads created from 14 days before the
booking to 10 minutes after; one lead → `contact`, none or several → unmatched. The leads' PII
is opened under `PANEL_DATA_KEY` only to compare.

**google_calendar, pulled** (`panel_server::google_calendar`). Every minute, each brand with a
refresh token whose sync is due (`GOOGLE_CALENDAR_SYNC_MINUTES`, 5): the refresh token → an
access token (`oauth2.googleapis.com/token`, cached until a minute before it expires) →
`GET www.googleapis.com/calendar/v3/calendars/{id}/events?singleEvents=true&showDeleted=true`,
first `timeMin` = now − 7 days, then `syncToken`; pages followed (≤ 40 of 250); 410 → full
pull. An event is a booking (`looks_like_booking`, conservative, **unconfirmed against a real
appointment-schedule event**): timed, single (no recurrence), `eventType` default, a guest
with an email who is not `self`, the organizer or a room, not declining, and a marker in the
description — "Booked by" / "Réservé par", a schedule link, or a phone label ("Téléphone",
"Phone number", …; the schedule's form must ask for it). The phone is the line after (or
beside) its label. Version: the event's `etag`; moved: the same id with a new start; a
`cancelled` item is passed on by id (a sync gives its id alone).

**Secrets and CLI.** `GOOGLE_OAUTH_CLIENT_ID` / `GOOGLE_OAUTH_CLIENT_SECRET` (one client,
Desktop kind; both or neither), `GOOGLE_CALENDAR_REFRESH_TOKEN_<BRAND>` (secret, scope
`calendar.events.readonly`) and `GOOGLE_CALENDAR_ID_<BRAND>` (optional, `primary`); a brand
without a token is not pulled; a token without the client fails the boot. `sites_panel booking
google-authorize <brand> [--port 8765]`: prints Google's consent URL (offline, prompt=consent,
PKCE, state), waits on `127.0.0.1:<port>` for the redirect, prints
`GOOGLE_CALENDAR_REFRESH_TOKEN_<BRAND>=…` once, for add-secrets. `sites_panel booking google-sync
<brand> [--full]`: one pull now.

**API** (`/api/v1`, Work; writes need `sa:sites:work:leads:edit`):

```text
POST /leads/{brand}/{lead}/booking         {action: "set", start_at, end_at?} | {action: "clear"}
                                           → 201 {event_id} (200 on an Idempotency-Key retry);
                                           400 bad instant; 404; 409 not from where it stands
POST /leads/{brand}/{lead}/booking/status  {status: done | no_show | canceled} → 201; 409 unless booked
GET  /bookings/unmatched?brand&limit       {bookings: [Booking]}, the next slot first, ≤ 200
POST /bookings/{id}/attach                 {lead} → 201 {event_id}; 404 no booking / no such lead
                                           of its brand; 409 attached to it by hand already
GET  /leads?booking=<status>               the leads whose booking stands there (none included)

Lead.booking: {status, provider, start_at, end_at, external_ref, match, preferred_date,
               preferred_part}
Booking:      {id, brand, provider, external_ref, status: booked | canceled, start_at, end_at,
               booked_at, last_event_at, lead_id: null, match: null,
               contact?: {name?, email?, phone?} (for a caller who sees PII)}
```

## Invariants

- **Sites trusts only the conductor's assertion.** Every route that reads or writes the
  business takes one (The conductor's assertion); the open ones are the landings' reads, the
  providers' webhooks (their own signatures), the bundle and `/health`. sites_panel calls no one
  for who a caller is.
- **The journal is append-only.** `events` rows are inserted once. Triggers refuse `DELETE`,
  `TRUNCATE`, and any `UPDATE` but of `status`/`status_reason`, which the rebuild rewrites when
  the registry has changed its mind about an event. Everything else is derived from it.
- **An event is its id.** The source issues a UUIDv7; the journal is keyed by it. A resend is
  `duplicate`; the same id with other content (compared as a MAC of the event in canonical
  form, so field spelling and key order do not matter, under a key derived from
  `PANEL_DATA_KEY`, so a dump cannot confirm a guess at the PII it covers) is `rejected`.
  Delivery is at-least-once on the sources' side, exactly-once in the journal.
- **Unknown types are kept, not dropped** (§3.2): stored as `unregistered`, not projected, and
  judged again by `sites_panel rebuild-projections` — which is how a type registered later
  picks up what arrived before it. Known types are checked strictly: unknown fields, a wrong
  vocabulary word, a lead event without a lead are rejected, and not journaled.
- **A key writes what it was registered for.** The conductor authenticates: it holds each key's
  kind and brands and checks its MAC over the body; its assertion names them and binds the
  body. Sites permits: `source.id` must be the key id, and an event claiming another source,
  another brand, or another kind (a site key cannot pass its events off as typed in by hand)
  is rejected; a kind no key is issued for (`panel`, `booking`) is refused whole. Events of
  kind `panel` are the manual ones (§10a), and every projection row carries `manual`.
- **A kind writes only its types** (`panel_core::event::may_write`): `lead.created` from `site`,
  `panel` or `bot`; `lead.contacted` and the calls from `panel` or `telephony`; quotes, wins,
  losses, completed jobs and payments from `panel` alone (payments are entered by hand).
  Unknown types are open to every kind. Checked on the grant at ingest and again by the
  registry, so the rebuild drops anything that slipped in.
- **A lead is created once.** Of several `lead.created`, the first journaled counts (by
  `received_at`, then id) and the others are left out of the projection, so a source cannot
  back-date a creation to take over someone else's lead.
- **Projections are functions of the journal.** A lead is never patched: every event about it
  recomputes its row from all its registered events (`panel_core::lead::fold`, ordered by
  `(occurred_at, id)`, so arrival order does not matter), inside the transaction that
  journaled the event. The rebuild runs the same code in one transaction, so it lands on the
  same state; the tests check that it does. Ingest and the rebuild do not interleave, nor two
  events of one lead: every transaction that writes begins `BEGIN IMMEDIATE`, taking SQLite's
  one write lock up front, so writers queue (up to `busy_timeout`, 10 s) and readers, in WAL,
  go on reading the last commit. Calls and payments are one row per event.
- **Stages move forward** through created → contacted → quoted → won → completed → paid; a
  `lead.lost` moves a lead to `lost` from anywhere, and later progress reopens it. Stage times
  are when each stage was first reached.
- **PII never sits in the clear.** `pii` is sealed per event (XChaCha20-Poly1305, the event id
  as associated data) under `PANEL_DATA_KEY`, whose fingerprint is stored beside every blob.
  The free text a customer typed goes in `pii`, not `properties`. The notes rendered from it are
  sealed the same way.
- **Reporting has no PII.** The `reporting_*` views over the projections and a daily ingest
  count select no `properties` or `pii_sealed`: what anything reading a copy of the database (a
  dashboard on the replica, an export) should be pointed at instead of the tables. SQLite has
  no roles to enforce that; whoever holds the file holds everything, sealed PII included —
  which is why PII is sealed.
- **The schema guards the journal.** `events` refuses DELETE and any UPDATE but of
  `status`/`status_reason` (compared NULL-safe, so writing a value back unchanged passes).
  Tables are STRICT, foreign keys enforced (`PRAGMA foreign_keys = ON` on every connection).
- **One file, migrated on open.** `PANEL_DB_PATH` names it (`/data/sites_panel.db` in the
  image); every command, `serve` included, creates it if missing and applies the migrations it
  lacks before anything else (`sites_panel migrate` does only that). A migration newer than
  the build is let be: that is a rollback onto a schema moved on. Timestamps are INTEGER
  microseconds since the epoch, days `YYYY-MM-DD` text, UUIDs 16-byte blobs, JSON text that
  must parse.
- **Secrets come from the environment only** (`PANEL_DATA_KEY`, `SENTRY_DSN`,
  `GOOGLE_OAUTH_CLIENT_SECRET`, `GOOGLE_CALENDAR_REFRESH_TOKEN_<BRAND>`), through
  `ev_lib::settings`; with `APP_ENV=production`, `PANEL_DB_PATH`, `PANEL_DATA_KEY` and
  `PANEL_ASSERTION_KEYS` are required at boot (`sites_panel --print-required-vars` lists them).

## Deploy requirements

- **One image, no public route.** The image carries the binary and the front end's bundle
  (`PANEL_MFE_DIR`; `mfe-sites.js` / `mfe-sites.css` `no-cache`, the hashed chunks `immutable`).
  The conductor reaches it by service DNS for `/api/sites/*` (→ `/api/v1/*`), ingest,
  `/api/hooks` and `/sites/mfe/*` (→ `/mfe/*`), and reads `/api/conductor/*`.
- **The contract** (`nix eval .#containers.<system>.sites_panel.contract`) names the port
  (59121), `/health`, `APP_ENV=production`, the variables required, secret and optional,
  `PANEL_DB_PATH`, the volume (`mounts = [ "/data" ]`) and the SQLite file on it to replicate
  (`sqlite = [ "/data/sites_panel.db" ]`), and the egress. `checks.contract-env` fails the
  flake when its list of required variables and the binary's `--print-required-vars` disagree.
- **Who may connect:** the NetworkPolicy admits the conductor, and the landings' pods for
  `/api/internal` (their places' settings, pricing and experiments' overrides; no assertion).
- **PostHog, outbound only.** With `POSTHOG_PROJECT_API_KEY` the pods need egress to
  `us.i.posthog.com:443` (or wherever `POSTHOG_HOST` points). The key is the project's public
  `phc_` key, not a secret. The experiments' links (`POSTHOG_PROJECT_ID`, `POSTHOG_APP_HOST`)
  are opened by the browser: no egress for them.
- **Google, outbound only.** With the booking pull configured the pods need egress to
  `oauth2.googleapis.com:443` and `www.googleapis.com:443`. `/api/hooks/booking` is public
  through the conductor (no push provider is registered yet: 404s), rate-limited per IP at
  the edge.
- **banking's ScopeLedger within reach.** With `SCOPE_LEDGER_ADDR` (`https://…:50054`) and the
  three `SCOPE_LEDGER_TLS_*_PEM_FILE` files mounted at `/etc/settings`, the pods need egress
  to that address. Banking must map the client certificate to its client and grant it
  `(client sa, scope sa, prefix sa:)`; until then every invoice is 503 `not_granted`.
- **One pod, a volume, litestream.** The database is the file on `/data`, a volume writable
  by uid 65534 that the cluster replicates off the pod with litestream (to R2) and restores
  on an empty volume. One writer: one replica, rolled out with `Recreate`, never two pods on
  the volume at once. No init container: `serve` migrates on start, so rolling out an image
  with a new migration applies it, and rolling the image back does not undo it.
