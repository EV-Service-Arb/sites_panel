//! kitstart's pricing fixtures, vendored by commit (`fixtures/pricing/SOURCE`): the contract
//! between the sites' `core/pricing` and the panel's port of it. Every valid model is accepted
//! (and survives a round trip through the wire shape), every invalid one refused, every case
//! priced to the cent — `null` included.
//!
//! To move to a newer kitstart: copy `ts/kitstart/test/fixtures/pricing` of that commit over
//! `fixtures/pricing`, write its sha in `SOURCE`, and make this pass.

use std::{fs, path::PathBuf};

use panel_core::pricing::{PricingInputs, PricingModel, price_of};
use serde_json::Value;

fn dir(sub: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pricing").join(sub)
}

fn read(path: &PathBuf) -> Value {
	let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
	serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// Every `*.json` of a fixture directory, by name.
fn models(sub: &str) -> Vec<(String, Value)> {
	let mut out: Vec<(String, Value)> = fs::read_dir(dir(sub))
		.unwrap_or_else(|e| panic!("listing {sub}: {e}"))
		.map(|entry| entry.unwrap().path())
		.filter(|p| p.extension().is_some_and(|e| e == "json"))
		.map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), read(&p)))
		.collect();
	out.sort_by(|a, b| a.0.cmp(&b.0));
	out
}

#[test]
fn the_fixtures_are_pinned() {
	let source = fs::read_to_string(dir("SOURCE")).unwrap();
	assert!(source.starts_with("EV-Service-Arb/sites_tools@"), "{source}");
	assert_eq!(models("valid").len(), 5, "the vendored set");
	assert_eq!(models("invalid").len(), 33, "the vendored set");
}

#[test]
fn every_valid_model_is_accepted_and_round_trips() {
	for (name, value) in models("valid") {
		let model = PricingModel::parse(&value).unwrap_or_else(|p| panic!("{name}: {p:?}"));
		assert_eq!(model.to_json(), value, "{name}: the wire shape, unchanged");
		assert_eq!(PricingModel::parse(&model.to_json()).unwrap(), model, "{name}");
	}
}

#[test]
fn every_invalid_model_is_refused() {
	for (name, value) in models("invalid") {
		let problems = PricingModel::parse(&value).expect_err(&name);
		assert!(!problems.is_empty(), "{name}");
		for p in &problems {
			assert!(p.path == "model" || p.path.starts_with("model."), "{name}: {p}");
		}
	}
}

#[test]
fn every_case_is_priced_to_the_cent() {
	let Value::Array(cases) = read(&dir("cases.json")) else { panic!("cases.json is a list") };
	assert_eq!(cases.len(), 30, "the vendored set");
	for case in cases {
		let name = case["name"].as_str().unwrap();
		let model = PricingModel::parse(&case["model"]).unwrap_or_else(|p| panic!("{name}: {p:?}"));
		let inputs: PricingInputs = serde_json::from_value(case["inputs"].clone()).unwrap();
		let want = case["cents"].as_i64();
		assert!(want.is_some() || case["cents"].is_null(), "{name}: cents is an integer or null");
		assert_eq!(price_of(&model, case["need"].as_str().unwrap(), &inputs), want, "{name}");
	}
}

/// The field each invalid fixture is refused at: the one thing its name says is wrong, at the
/// path the editor is told.
#[test]
fn each_invalid_model_is_refused_for_what_its_name_says() {
	let want = [
		("currency-usd", "model.currency"),
		("dearest-adds-over-cap", "model.needs.standard"),
		("dearest-multiply-over-cap", "model.needs.standard"),
		("discount-over-100-percent", "model.inputs[3].options[0].discountBp"),
		("duplicate-input", "model.inputs"),
		("duplicate-option", "model.inputs[1].options"),
		("empty-labels", "model.inputs[0].labels"),
		("extra-effect-field", "model.inputs[1].options[1].discountBp"),
		("fixed-with-a-base", "model.needs.windows.baseCents"),
		("float-cents", "model.needs.standard.baseCents"),
		("format-2", "model.format"),
		("label-blank", "model.inputs[0].options[0].labels.fr"),
		("label-locale-not-a-locale", "model.inputs[0].labels.french"),
		("label-too-long", "model.inputs[0].options[0].labels.fr"),
		("minimum-negative", "model.minimumCents"),
		("missing-needs", "model.needs"),
		("multiply-over-x10", "model.inputs[0].options[2].multiplyBp"),
		("need-input-twice", "model.needs.standard.inputs"),
		("need-kind-quote", "model.needs.windows.kind"),
		("need-not-a-slug", "model.needs.Standard"),
		("need-unknown-input", "model.needs.standard.inputs[4]"),
		("negative-add", "model.inputs[1].options[0].addCents"),
		("no-options", "model.inputs[0].options"),
		("not-an-object", "model"),
		("option-id-not-a-slug", "model.inputs[1].options[1].id"),
		("round-to-zero", "model.roundToCents"),
		("string-cents", "model.minimumCents"),
		("too-many-need-inputs", "model.needs.standard.inputs"),
		("unknown-field", "model.note"),
		("unknown-kind", "model.inputs[3].kind"),
		("valid-from-a-datetime", "model.validFrom"),
		("valid-from-not-a-day", "model.validFrom"),
		("wrong-effect-field", "model.inputs[1].options[1].addCents"),
	];
	let fixtures = models("invalid");
	assert_eq!(fixtures.len(), want.len(), "a fixture added upstream needs its line here");
	for ((file, value), (name, path)) in fixtures.iter().zip(want) {
		assert_eq!(file.as_str(), format!("{name}.json"));
		assert_eq!(PricingModel::parse(value).unwrap_err()[0].path, path, "{name}");
	}
}
