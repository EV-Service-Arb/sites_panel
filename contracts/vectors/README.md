# The site ↔ panel contract's vectors

What `panel_core` / `panel` (Rust) and `@evinvest/sites-contract` (`ts/sites-contract`) are
both held to, read in place by both suites (`crates/panel_core/tests/vectors.rs`,
`crates/panel/tests/{booking,ingest}.rs`, `ts/sites-contract/test/vectors.test.ts`). A vector
changed is the contract changed, on both sides at once; a site sends a new shape only once a
panel that accepts it is live.

```text
pricing/     a brand's price list: valid, invalid (with their problems), locales, priced cases
booking/     a place's booking config and its URL rule; booking.requested's properties
place.json   a place's live settings, field by field: what is kept, what is refused and why
phone.json   a typed phone number → E.164, or null
ids.json     the ids and refs a lead's events carry: opaque ids, analytics ids, message refs
ingest/      the bodies a site sends (`siteEvent`), byte for byte, each accepted by the panel;
             declarations.json: an experiment as declared, taken or refused
```

Problems are `{ path, code }`, compared as sets: JSON objects are read sorted in Rust and in
insertion order in TS, so the order may differ, never which problems there are.
