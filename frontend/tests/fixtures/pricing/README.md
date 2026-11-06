# Pricing fixtures (vendored)

Copied verbatim from EV-Service-Arb/sites_tools `71d6451`,
`ts/kitstart/test/fixtures/pricing/{valid,invalid}/` — the contract between
kitstart and the panel (see that directory's README). `tests/pricing-model.test.ts`
holds the editor's validator (`src/entities/pricing/model/check.ts`) to them.
`cases.json` is not vendored: the front end never prices a model, the server does.

To update: copy the directories again from the new sha and change it here.
