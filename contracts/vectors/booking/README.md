# Booking vectors

- `valid/*.json` — place booking configs (`PlaceLive.booking`, a place
  setting) every checker accepts and keeps as they came.
- `invalid/*.json` — `{ problems: [{ path, code }], config }`: refused for
  exactly these problems; the path is below the setting (`""`, `default`,
  `providers.cal_com.url`), the codes `BOOKING_CODES`. The file name says the
  one thing wrong.
- `urls.json` — `[{ provider, url, problem }]`: a page's URL by provider, its
  code or `null`.
- `requested/valid/*.json`, `requested/invalid/*.json` — `booking.requested@1`
  properties taken / refused.

## The config

`{ "default": <provider>, "providers": { <provider>: { "url" } } }`. The
providers are a closed set — `manual | link | google_calendar | cal_com`;
unknown fields are refused, not ignored. (`calendly` is the next one: its own
URL rule, nothing else changes.)

- `manual` — no page: the site promises a call to set the slot, the operator
  records it. Always available, so it is never a key of `providers`.
- `link` — any booking page that passes the URL rule.
- `google_calendar` — the URL rule, plus: the host is `calendar.app.google`
  with a non-empty path (a short link), or `calendar.google.com` with a path
  under `/calendar/appointments/` (something after it).
- `cal_com` — the URL rule, plus: the host is one of `BOOKING_LIMITS.calComHosts`
  (`cal.evinvest.ltd`, `cal.com`; exact match, a subdomain is another host), and the path is
  exactly `/<user>/<event>`, each segment `[A-Za-z0-9][A-Za-z0-9._-]{0,99}` —
  no trailing slash, no percent-escape, no dot segment.
- `default` — `manual`, or a key of `providers`.

## The URL rule (normative, every provider with a page)

1. A string of at most 2048 characters, printable ASCII only (`0x21`–`0x7e`):
   no space, tab, line break or non-ASCII character.
2. Starts with the literal, lowercase `https://`. No other scheme.
3. No `#` anywhere (no fragment, not even an empty one); no `\`.
4. The authority (up to the first `/` or `?`) has no `@` (userinfo, even
   empty) and no `:` (a port, even empty) and does not start with `[`.
5. The host is a dotted DNS name: at least two labels, each
   `[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?`, at most 253 characters, no
   trailing dot — and its last label is not a number (`\d+` or `0x…`), which
   a WHATWG parser would read as an IPv4 address. Hosts compare lowercase.
6. A query is allowed and kept.

## `booking.requested@1` (source kind `site`)

Sent by the site after `lead.created`, through the same outbox. Properties,
snake_case, unknown fields and nulls refused:

- `lead_ref` — `lead-<row>-<8 hex>`, row ≥ 1, lowercase hex. Required.
- `provider` — `manual | link | google_calendar | cal_com`, the one chosen.
  Required.
- `preferred_date` — `YYYY-MM-DD`, a real calendar day. `manual` only.
- `preferred_part` — `morning | afternoon | evening`. `manual` only.

No free text: nothing a person typed travels here.
