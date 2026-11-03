# Sites front end: the `mfe-sites` bundle

The sites screens of `SA-PANEL-SPEC.md` §10 as one self-registering ESM bundle the conductor mounts: Feature-Sliced
Design, `@evinvest/uikit`, React in the elements' light DOM (the org's MFE pattern, concierge's `defineElement`).

```text
<mfe-sites-nav  api="/api/sites" base="/sites" csrf-cookie="…">   the Sites dropdown: Work (Leads● Locations Pricing
                                                                    Invoices), Analysis (Overview Experiments), by /me
<mfe-sites-panel api="/api/sites" base="/sites" csrf-cookie="…">  the page: one screen per route under `base`

/sites               → /sites/leads?stage=created (Work), /sites/overview (Analysis only), or "no access"
/sites/leads  /sites/places  /sites/pricing  /sites/invoices      sa:sites:work:read
/sites/overview  /sites/experiments                              sa:sites:analysis:read
anything else under /sites                                       "nothing here"

window events (CONTRACT §9)
  in:  sa:live {server message}   sa:live-status {status}
  out: sa:live-status-request     sa:title-count {count} (nav only)   sa:unauthenticated   ev:navigate {href}
```

```text
src/app/             the two elements: defineElement, the /me gate, the page's routes (screens are lazy chunks)
src/views/           one slice per screen, plus nav/ (the dropdown, its marks)
src/features/        call-lead · move-stage · record-payment · create-lead · lead-filters · funnel-filters ·
                     edit-place-settings · revert-place-change · withdraw-place · add-place · configure-experiment ·
                     edit-pricing · create-invoice · pay-invoice-element · …
src/entities/        session (/me) · lead · funnel · experiment · place · pricing · booking · invoice
src/shared/          api/ (fetch onto `api`, CSRF, the gate's answers), config/host.ts (the attributes, one set per
                     page), lib/router.ts (one location for both elements), lib/live/ (the host's relayed socket), i18n/, ui/
messages/            en.json (the source of keys) and ru.json
scripts/build.mjs    esbuild + Tailwind → dist/mfe-sites.{js,css} and dist/chunks/*-[hash].js
scripts/dev-stub.ts  the conductor and sites stood in for: dev/index.html, dist/, /api/sites, the live socket
tests/               vitest, in Node: the rules the screens obey live in plain modules
```

## Running it

```sh
npm ci
npm run dev          # build, then on STUB_PORT (the dev shell's) — open /sites; STUB_ROLE=admin, STUB_ME=401|403|503,
                     # STUB_MIN_SAMPLE=2, STUB_POSTHOG=off, STUB_PLACES_CONFLICT=1, STUB_LIVE=off,
                     # STUB_LIVE_EVERY=5, STUB_BOOKING_CONFLICT=1 (scripts/dev-stub.ts says what each does)
```

After a change under `src/`, `npm run build` and reload. Checks: `npm run typecheck`, `npm run lint`, `npm test`,
`npm run build`.

## How it is served

sites serves `dist/` at `/mfe/*` (`PANEL_MFE_DIR`), the conductor forwards it as `/sites/mfe/*`: the entry
`mfe-sites.js` and `mfe-sites.css` under stable names (`no-cache`), the chunks content-hashed (`immutable`). The
entry adds `mfe-sites.css` as a `<link>` beside itself; the host moves it under its own layers. Utilities are
`important` and scoped to the two tags (`scripts/scope-css.mjs`): the host's preflight must not beat them, and they
must not reach the host's own nodes. No preflight and no fonts ship: the host has both.

The pay buttons on Invoices load banking's wallet bundle from evinvest.ltd, pinned by SRI
(`shared/config/wallet-bundle.ts`): the conductor's CSP must name it.

## Live updates

The conductor owns the one socket and relays it on `window` (`sa:live`, `sa:live-status`); each element asks for the
status as it mounts (`sa:live-status-request`), since the socket was up before it. A status back at `live` after a
drop re-reads every mounted screen (events may have been missed), and so does every minute `offline`.

Reads follow topics through `useResource(…, { live })`, debounced (a batch import sends one `changed` per event). What
the screens do with it:

- **Leads**: arrivals wait in a "New leads: N — Show" banner rather than moving
  the rows; rows on screen update in place and glow; a row that left the filter
  is re-read and kept. The open card follows its lead and notes a change made
  elsewhere (the event names no person, so the note says when and what).
- **Locations**: an open form is never overwritten — with no edits it takes the
  newer data, with edits it says who saved and offers to load it (saving would
  be refused with 409 anyway).
- **Nav**: Leads counts new leads since the screen was last in front of you (the
  baseline kept in localStorage per `sub`), Locations the places changed,
  Experiments a dot; opening the screen clears it. While the tab is not focused
  the nav sends the count of new leads (`sa:title-count`) for the host's title.

## Place settings

A location card opens its live site data (`SA-PANEL-PLACE-SETTINGS-SPEC.md`):
phone, WhatsApp, opening hours and service area, which the site merges over its
baked config within its 600 s fetch TTL. The form edits only those four; any
other live field (address, geo, photo, landmark, rating) goes back as it came,
since a save is a full replace guarded by `expected_updated_at` (409 → reload).
A 422 names its fields and the form shows each reason on its field. The history
reads each change as before → after, with a revert guarded the same way; an admin can withdraw a
point (the site then answers 404 for it) and add one by hand, under a brand some place or price list already names.
An operator reads.

The preview of the contact channels assumes the places' hours are Paris time
(`entities/place/config/time-zone.ts`): the live data carries no zone.

## What the API does not answer yet

- **Stages 1–2** (Maps) arrive with the GBP import; until then the overview says so
  rather than showing zeros.
- **Stages 3–4** (site visits, contact intents) and the experiments' results are
  PostHog's and are read there; the panel does not count them again. The
  Experiments screen is config: what each landing declared, the split it runs,
  and for an admin the kill switch, the weights and the holdout, with a link to
  the experiment's funnel in PostHog when the backend has a project configured.
- **Lost reasons** are a slug the backend takes freely; the panel offers a fixed
  list (`features/move-stage/model/moves.ts`) so the reports can group by it.
