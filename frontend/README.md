# Sites front end: the `mfe-sites` bundle

The sites screens of `SA-PANEL-SPEC.md` §10 as one self-registering ESM bundle the conductor mounts: Feature-Sliced
Design, `@evinvest/uikit`, React in the elements' light DOM (the org's MFE pattern, concierge's `defineElement`).

```text
<mfe-sites-nav  api="/api/sites" base="/sites" csrf-cookie="…">   the Sites dropdown: Work (Leads● Locations Pricing
                                                                    Invoices), Analysis (Overview Experiments), by /me
<mfe-sites-panel api="/api/sites" base="/sites" csrf-cookie="…">  the page: one screen per view under `base`

the views are the document's x-views (VIEWS, panel_server::views::View): each one's path, query keys, needs and key;
`parse`/`href` (@evinvest/views) read and write them, `mayOpen` gates each screen
/sites               → /sites/leads?stage=created (Work), /sites/overview (Analysis only), or "no access"
an href no view takes (another path, a key or a word it does not know)                    "nothing here"

window events (CONTRACT §9)
  in:  ev:live {server message}   ev:live-status {status}   sa:gmbs {ids: number[] | null}
  out: ev:live-status-request     sa:gmbs-request   sa:title-count {count} (nav only)   sa:unauthenticated   ev:navigate {href}
  from the nav element: ev:keymap — each listed view's own key, under the scope its host assigns it
```

```text
src/app/             the two elements: defineElement, the /me gate, a screen per view (lazy chunks)
src/views/           one slice per screen, plus nav/ (the dropdown, its marks)
src/features/        call-lead · move-stage · record-payment · create-lead · lead-filters · funnel-filters ·
                     edit-place-settings · revert-place-change · withdraw-place · add-place · configure-experiment ·
                     edit-pricing · create-invoice · pay-invoice-element · …
src/entities/        session (/me) · lead · funnel · experiment · place · pricing · booking · invoice
src/shared/          api/ (`api()`: @evinvest/api over the committed openapi.json, its schema.ts; CSRF on writes),
                     config/host.ts (the attributes, one set per page), lib/router.ts (one location for both elements),
                     i18n/, ui/
messages/            en.json (the source of keys) and ru.json
scripts/build.mjs    esbuild + Tailwind → dist/mfe-sites.{js,css} and dist/chunks/*-[hash].js
tests/               vitest, in Node: the rules the screens obey live in plain modules
```

## Running it

```sh
npm ci
```

Against the real stack: site_conductor's `nix run .#local-stack` (`SITES_DIR` names this checkout, `SITES_MFE_DIR` its
`dist/`). After a change under `src/`, `npm run build` and reload. Checks: `npm run typecheck`, `npm run lint`, `npm test`,
`npm run build`.

## How it is served

sites serves `dist/` at `/mfe/*` (`PANEL_MFE_DIR`), the conductor forwards it as `/mfe/sites/*`: the entry
`mfe-sites.js` and `mfe-sites.css` under stable names (`no-cache`), the chunks content-hashed (`immutable`). The
entry adds `mfe-sites.css` as a `<link>` beside itself; the host moves it under its own layers. Utilities are
`important` and scoped to the two tags (`scripts/scope-css.mjs`): the host's preflight must not beat them, and they
must not reach the host's own nodes. No preflight and no fonts ship: the host has both.

The pay buttons on Invoices load banking's wallet bundle from evinvest.ltd, pinned by SRI
(`shared/config/wallet-bundle.ts`): the conductor's CSP must name it.

## The host's selection

The host's outline selects GMBs (`sa:gmbs`; `null` is every one, `[]` none); each element asks as it connects
(`sa:gmbs-request`) and shows nothing until told (`app/gmb-gate.tsx`). Every screen narrows to the places linked to
them (`places.gmb_id`; a place linked to none is outside any selection): the leads, counts and funnels by the API's
`gmb`, the brand-level lists (experiments, pricing, bookings without a lead) to the brands of the selected places, and
the nav's marks to both. Invoices name no place. The element never sets the selection: the outline is its one editor.

## Live updates

The conductor owns the one socket and relays it on `window` (`ev:live`, `ev:live-status`); `@evinvest/api/react`
asks for the status once per page (`ev:live-status-request`), since the socket was up before it. A status back at
`live` after a drop re-reads every mounted read (events may have been missed), and so does every minute `offline`.

Every read follows the tables its answer says it read (`x-sa-reads`): `useResource(api(), path, init)`, or `useLive` on
an answer (the leads queue's first page), debounced (a batch import sends one change per row). What the screens do
with it:

- **Leads**: arrivals wait in a "New leads: N — Show" banner rather than moving
  the rows; rows on screen update in place and glow; a row that left the filter
  is re-read and kept. The open card follows its lead and notes a change made
  elsewhere (the change names no lead, so any write to the leads re-reads it;
  the note says when and what).
- **Locations**: an open form is never overwritten — with no edits it takes the
  newer data, with edits it says who saved and offers to load it (saving would
  be refused as `stale` anyway).
- **Nav**: Leads counts new leads since the screen was last in front of you (the
  baseline kept in localStorage per `sub`), Locations the places changed since,
  Experiments a dot, each read off the times its list answers with (`changed_at`,
  an experiment's declaration and override); opening the screen clears it. While
  the tab is not focused the nav sends the count of new leads (`sa:title-count`)
  for the host's title.

## Place settings

A location card opens its live site data (`SA-PANEL-PLACE-SETTINGS-SPEC.md`):
phone, WhatsApp, opening hours and service area, which the site merges over its
baked config within its 600 s fetch TTL. The form edits only those four; any
other live field (address, geo, photo, landmark, rating) goes back as it came,
since a save is a full replace guarded by `expected_updated_at` (`stale` → reload).
The form runs the contract's own checks before saving; a refusal still shows the
server's own words above it. The history
reads each change as before → after, with a revert guarded the same way; an admin can withdraw a
point (the site then answers 404 for it) and add one by hand, under a brand some place or price list already names.
Where a place books through Google Calendar, an admin connects the brand's calendar from there (Google's consent,
back to the same page). An operator reads.

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
- **Lost reasons** are the document's `LostReason` words, kept few so the reports compare;
  a lead lost before them shows its slug as it came.
