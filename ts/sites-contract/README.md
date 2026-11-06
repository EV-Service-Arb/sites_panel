# @evinvest/sites-contract

The Service-Arb sites ↔ panel contract, in TS: what a landing reads from the panel and sends
to it. Its source is the panel — `crates/panel_core` and `contracts/proto/sa/v1` in
[EV-Service-Arb/sites_panel](https://github.com/EV-Service-Arb/sites_panel) — and this is its
only TS form. No runtime dependencies, no Node imports: it is bundled into landing pages and
the panel's front end.

```text
src/generated/contract.ts  panel_core's shapes, vocabularies and limits (`nix run .#gen`)
src/generated/wire.ts      the protos as protojson: the envelope, every type's properties
src/*.ts                   the one TS implementation of what both sides run, held to
                           contracts/vectors with panel_core:
  pricing    checkPricingModel, priceOf
  booking    checkBookingConfig, checkBookingUrl, bookingRequested, isLeadRef
  place      checkPlaceLive: a place's live settings, field by field
  phone      normalizePhone, phoneProblem
  ingest     siteEvent, ingestBody, uuidV7, isOpaqueId: the only way a site builds an event
  experiments parseLiveExperiments, declarationProblem
```

Checks answer `{ path, code }` problems, never words: a site logs them, the panel's editor says
each code in its own copy.

Release: bump `version`, commit, push the tag `@evinvest/sites-contract-v<version>`; CI
(`release-npm.yml`) publishes it.
