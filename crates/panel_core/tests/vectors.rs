//! `contracts/vectors`: what `@evinvest/sites-contract` is held to as well, read in place. A
//! vector changed is the contract changed, on both sides at once.

use std::{
	collections::BTreeSet,
	fs,
	path::{Path, PathBuf},
};

use panel_core::{
	booking::{self, PageProvider, check_config, check_url},
	experiment::Declaration,
	fact::{AnalyticsId, MessageRef},
	ids::is_opaque,
	phone,
	place::PlaceLive,
	pricing::{PricingInputs, PricingModel, price_of},
};
use serde_json::Value;

fn dir(sub: &str) -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/vectors").join(sub)
}

fn read(path: &Path) -> Value {
	let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
	serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// Every `*.json` of a vector directory, by name.
fn each(sub: &str) -> Vec<(String, Value)> {
	let mut out: Vec<(String, Value)> = fs::read_dir(dir(sub))
		.unwrap_or_else(|e| panic!("listing {sub}: {e}"))
		.map(|entry| entry.unwrap().path())
		.filter(|p| p.extension().is_some_and(|e| e == "json"))
		.map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), read(&p)))
		.collect();
	out.sort_by(|a, b| a.0.cmp(&b.0));
	assert!(!out.is_empty(), "{sub} holds no vector");
	out
}

fn list(file: &str) -> Vec<Value> {
	let Value::Array(items) = read(&dir(file)) else { panic!("{file} is a list") };
	items
}

/// The `[{path, code}]` a vector expects, as a set: which problems there are, not their order.
fn problems(v: &Value) -> BTreeSet<(String, String)> {
	v.as_array()
		.expect("problems is a list")
		.iter()
		.map(|p| (p["path"].as_str().unwrap().to_owned(), p["code"].as_str().unwrap().to_owned()))
		.collect()
}

#[test]
fn every_valid_model_is_accepted_and_written_back_unchanged() {
	for (name, value) in each("pricing/valid") {
		let model = PricingModel::parse(&value).unwrap_or_else(|p| panic!("{name}: {p:?}"));
		assert_eq!(model.to_json(), value, "{name}: the wire shape, unchanged");
	}
}

#[test]
fn every_invalid_model_is_refused_for_what_it_says() {
	for (name, v) in each("pricing/invalid") {
		let got: BTreeSet<_> = PricingModel::parse(&v["model"])
			.expect_err(&name)
			.into_iter()
			.map(|p| (p.path, p.code.as_str().to_owned()))
			.collect();
		assert_eq!(got, problems(&v["problems"]), "{name}");
	}
}

#[test]
fn a_site_wants_every_label_in_its_locales() {
	for case in list("pricing/locales.json") {
		let locales: Vec<String> = serde_json::from_value(case["locales"].clone()).unwrap();
		let got: BTreeSet<_> = match PricingModel::parse_for(&case["model"], &locales) {
			Ok(_) => BTreeSet::new(),
			Err(p) => p.into_iter().map(|p| (p.path, p.code.as_str().to_owned())).collect(),
		};
		assert_eq!(got, problems(&case["problems"]), "{}", case["name"]);
	}
}

#[test]
fn every_case_is_priced_to_the_cent() {
	for case in list("pricing/cases.json") {
		let name = case["name"].as_str().unwrap();
		let model = PricingModel::parse(&case["model"]).unwrap_or_else(|p| panic!("{name}: {p:?}"));
		let inputs: PricingInputs = serde_json::from_value(case["inputs"].clone()).unwrap();
		let want = case["cents"].as_i64();
		assert!(want.is_some() || case["cents"].is_null(), "{name}: cents is an integer or null");
		assert_eq!(price_of(&model, case["need"].as_str().unwrap(), &inputs), want, "{name}");
	}
}

#[test]
fn every_valid_booking_is_kept_as_it_came() {
	for (name, config) in each("booking/valid") {
		let checked = check_config(&config).unwrap_or_else(|p| panic!("{name}: {p:?}"));
		assert_eq!(serde_json::to_value(checked).unwrap(), config, "{name}");
	}
}

#[test]
fn every_invalid_booking_is_refused_for_what_it_says() {
	for (name, v) in each("booking/invalid") {
		let got: BTreeSet<_> = check_config(&v["config"]).expect_err(&name).into_iter().map(|p| (p.path, p.code.as_str().to_owned())).collect();
		assert_eq!(got, problems(&v["problems"]), "{name}");
	}
}

#[test]
fn every_booking_url_by_provider() {
	for u in list("booking/urls.json") {
		let provider = PageProvider::parse(u["provider"].as_str().unwrap()).unwrap();
		let url = u["url"].as_str().unwrap();
		let got = check_url(provider, url).err().map(booking::Code::as_str);
		assert_eq!(got, u["problem"].as_str(), "{provider} {url}");
	}
}

#[test]
fn every_phone_as_the_funnel_reads_it() {
	for p in list("phone.json") {
		let raw = p["raw"].as_str().unwrap();
		assert_eq!(phone::normalize(raw).as_deref(), p["e164"].as_str(), "{raw:?}");
	}
}

/// JSON numbers compared as numbers: the panel writes a float `45.0` where a site wrote `45`.
fn same(a: &Value, b: &Value) -> bool {
	match (a, b) {
		(Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
		(Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y)),
		(Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
		_ => a == b,
	}
}

#[test]
fn every_place_setting_is_kept_or_refused_by_its_rule() {
	for case in list("place.json") {
		let name = &case["name"];
		let (live, found) = PlaceLive::check(&case["settings"]);
		let live = serde_json::to_value(live).unwrap();
		assert!(same(&live, &case["live"]), "{name}: kept {live}");
		let found: BTreeSet<_> = found.into_iter().map(|p| (p.path, p.reason.as_str().to_owned())).collect();
		assert_eq!(found, problems(&case["problems"]), "{name}");
	}
}

#[test]
fn every_declared_experiment_is_taken_or_refused() {
	for case in list("ingest/declarations.json") {
		let d = &case["declaration"];
		let parsed = Declaration::parse(
			d["key"].as_str().unwrap(),
			serde_json::from_value(d["variants"].clone()).unwrap(),
			serde_json::from_value(d["weights"].clone()).unwrap(),
			d["enabled"].as_bool().unwrap(),
			d["holdout"].as_f64(),
			d["summary"].as_str().map(str::to_owned),
		);
		assert_eq!(parsed.is_ok(), case["ok"].as_bool().unwrap(), "{}: {parsed:?}", case["name"]);
	}
}

#[test]
fn every_id_and_ref_a_lead_carries() {
	let ids = read(&dir("ids.json"));
	let each = |kind: &str, ok: &dyn Fn(&str) -> bool| {
		for v in ids[kind].as_array().unwrap() {
			let value = v["value"].as_str().unwrap();
			assert_eq!(ok(value), v["ok"].as_bool().unwrap(), "{kind} {value:?}");
		}
	};
	each("opaque", &is_opaque);
	each("analyticsId", &|s| AnalyticsId::parse(s).is_ok());
	each("messageRef", &|s| MessageRef::parse(s).is_ok());
	each("leadRef", &booking::is_lead_ref);
}

#[test]
fn every_preferred_day_in_its_window() {
	for case in list("booking/window.json") {
		let day = |k: &str| case[k].as_str().unwrap().parse::<jiff::civil::Date>().unwrap();
		assert_eq!(booking::preferred_date_in_window(day("date"), day("today")), case["ok"].as_bool().unwrap(), "{case}");
	}
}
