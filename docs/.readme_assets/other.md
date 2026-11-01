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

A landing (kitstart) bakes its places into its build, and lays over them what the panel
answers for each one: phones, WhatsApp, the Telegram bot and the messenger switches, opening
hours, service area, and for storefronts an address, a pin, a photo, a landmark, a rating
(kitstart's `PlaceLive`). Changing a number is
an edit in the panel or a `panel place set`, not a release. A site fetches at most every
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
every change, from the panel or the CLI (`by = cli`), is journaled with what was before and
after, and can be reverted. The settings are checked as kitstart reads them, refused field
by field (`422`) where kitstart would quietly drop them. The session API is in
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
