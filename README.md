# sites_panel
![Minimum Supported Rust Version](https://img.shields.io/badge/nightly-1.100+-ab6000.svg)
![Lines Of Code](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/sites_panel-loc.json)
![Build time](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/sites_panel-build.json)
![Test time](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/sites_panel-test.json)
<br>
[![errors](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/sites_panel-errors.json)](https://github.com/EV-Service-Arb/sites_panel/actions/workflows/errors.yml?query=branch%3Amain)
[![warnings](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/sites_panel-warnings.json)](https://github.com/EV-Service-Arb/sites_panel/actions/workflows/warnings.yml?query=branch%3Amain)

The Service-Arb panel (`sa.evinvest.ltd`): where the funnel of the Service-Arb brands — from a
lead to a payment — is recorded and read. The plan is `SA-PANEL-SPEC.md` (§ references in the
code point there); this repository holds its backend.

What exists so far is ingest. Sources (the landings, and later review_archive and a GBP
import, the panel's own screens) send `sa.funnel.v1` events — a versioned protobuf
contract, spoken as protojson — to `POST /api/ingest/v1/events`, signed with a per-source HMAC
key that may write only for its own brands. Every accepted event goes into an append-only
journal in SQLite (one file, replicated off the pod by litestream), PII sealed apart; the
funnel's projections (`leads` with their stages, `calls`, `payments`) are derived from it and
can be rebuilt from it at any time. The `reporting_*` views expose them without PII.

People sign in through concierge (the panel is its relying party, client `sa`): any active
account signs in, and the `sa` permissions it holds (the panel publishes their catalog; the
aliases `sa:operator` and `sa:admin` bundle them) open its sections. `/api/v1` is the
operator API the panel's front end works through — leads and their stages, semi-manual
calls, payments typed in by hand, the funnel, and (admins) the sources.

A Telegram bot notifies each user in a private chat they link from their profile — a new
lead, with buttons that record "taken" and "no answer" as the operator API would; a lead past
its contact SLA; for admins, payments and a source gone silent — through an outbox in the
same database, paced to Telegram's limits.

Admins edit each place's live settings — the phones, hours and service area the landings
show — and each brand's price list and experiments (a kill switch, the weights, the holdout
over what the landing declares), and the landings read them from the panel instead of a
release. What PostHog counts (visits, intents, the experiments' numbers) is looked at in
PostHog; the panel sends it each lead's life after the form, without PII.

Not here yet: the GBP import. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for where things live.
<!-- markdownlint-disable -->
<details>
<summary>
<h2>Installation</h2>
</summary>

nix build


</details>
<!-- markdownlint-restore -->

## Usage
```sh
# Settings come from the environment only: PANEL_DB_PATH, the SQLite file (created and migrated
# by whichever command opens it first), and PANEL_DATA_KEY (64 hex characters) that seals PII
# and the queued notes. `sites_panel --print-required-vars` lists what production needs.
export PANEL_DATA_KEY="$(sites_panel gen-data-key)"
export PANEL_DB_PATH=./panel.db

# Every command migrates on open; this one does nothing else.
sites_panel migrate

# Source keys are the conductor's (site_conductor, `POST /api/v1/sources`); none are issued here.

# HTTP behind the conductor, trusting only its assertions: PANEL_ASSERTION_KEYS are its public
# keys. TELEGRAM_LOCALE=ru|en is the notes' buttons' language.
sites_panel serve --bind 127.0.0.1:59120       # or PANEL_BIND

# leads, calls and payments again from the journal, against the registry as it is now
sites_panel rebuild-projections
```

## Sending events

```text
POST /api/ingest/v1/events
x-sa-key-id:    aquafix-site
x-sa-timestamp: 1790762400                       unix seconds; ±5 minutes of the panel's clock
x-sa-signature: hex(HMAC-SHA256(secret, "sa-ingest/v1." + <x-sa-timestamp> + "." + <raw body>))

{"events": [ …1 to 500 sa.v1.Event, protojson… ]}
```

The answer is `207` with a verdict per event, in order: `accepted`, `duplicate` (that id is
journaled already), or `rejected` with a reason. A type the panel does not know yet is
`accepted` and stored, and projected once it is registered. `401` refuses the whole batch
(key, signature or timestamp), `400` a body that is not a batch. The contract is
[`contracts/proto/sa/v1/events.proto`](contracts/proto/sa/v1/events.proto).

A messenger bot (a key of kind `bot`) sends its leads and the fact that a customer wrote the
same way, and looks a lead up by the ref the customer brought:
`GET /api/ingest/v1/leads/by-ref/<brand>/<ref>`, signed over `GET <path>` in place of a body. See
[docs/BOT-API.md](docs/BOT-API.md).

## Place settings

A landing (sites-tools) bakes its places into its build, and lays over them what the panel
answers for each one: phones, WhatsApp, the Telegram bot and the messenger switches, opening
hours, service area, a review link, and for storefronts an address, a pin, a photo, a landmark,
a rating (`PlaceLive`). Changing a number is
an edit in the panel, not a release. A site fetches at most every
10 minutes; the panel being down or slow only delays a change, the site serves what it baked.

```text
GET /api/internal/brands/<brand>/locations/<slug>?locale=fr
    200 {"phone": "+33…", "hours": [{"days": ["Monday"], "opens": "08:00", "closes": "19:00"}], …}
        only the fields set; {} for a place without settings or unknown to the panel
    404 {"error": "not_found"}  only for a place an admin withdrew: the site 404s that page
```

A site is given `SITES_PANEL_INTERNAL_URL` (this service's origin plus `/api/internal`, derived by
devops from the contracts: `provides.internal` here, `consumes` on the landing) and appends
`/brands/<brand>/…`.

`/api/internal` has no session and is not published: the public IngressRoute must exclude it,
and the NetworkPolicy lets the landings' pods in. Admins edit in the panel (operators read);
every change is journaled with what was before and
after, and can be reverted. The settings are checked field by field by the rules a site reads
them with (`@evinvest/sites-contract`), refused (`422`) where a site would quietly drop them. The session API is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#place-settings).

## Running it locally

```sh
nix run .#local-stack    # the panel on :59120 (signed in as a dev admin) and the aquafix and vifnet
                         # landings from the checkouts beside it, wired together; Ctrl-C stops all
```

A lead posted on a local site lands in the local panel, a phone edited in the panel shows on
the site. `PANEL_DEV_SIGN_IN=sa:admin|sa:operator|<permissions>|none` stands in for concierge, in development on
loopback only. The prerequisites, the end-to-end check and troubleshooting are in
[docs/LOCAL.md](docs/LOCAL.md).

## Tests

`cargo test` runs everything, here and in CI: each database test gets its own throwaway SQLite
file in the temp directory (`panel_test_*.db`), removed when it ends. Nothing to set up.


<br>

<sup>Links for devs:</sup>
- <sup><a href="./docs/ARCHITECTURE.md">architecture</a></sup>
- <sup><a href="https://github.com/EV-Main/.github/blob/main/code/STYLE.md">style</a></sup>
- <sup><a href="https://github.com/EV-Main/.github/tree/main/code/conventions">conventions</a></sup>
- <sup><a href="https://github.com/EV-Main/.github/blob/main/code/conventions/git">contributing</a></sup>

#### License

<sup>
	Licensed under <a href="LICENSE">Blue Oak 1.0.0</a>
</sup>

<br>

<sub>
	Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be licensed as above, without any additional terms or conditions.
</sub>

