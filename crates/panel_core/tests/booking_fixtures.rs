//! kitstart's booking fixtures, vendored by commit (`fixtures/booking/SOURCE`): the contract
//! between the sites' `core/booking` and the panel's validators. Every valid place config is
//! accepted (and stored as it came), every invalid one refused; the Cal.com hosts are
//! `rules.json`'s. The `requested/*` properties are judged through the engine's registry
//! (`crates/panel/tests/booking.rs`); `choose.json` and `hrefs.json` are the site's to keep —
//! the panel neither chooses a provider nor builds a link.
//!
//! To move to a newer kitstart: copy `ts/kitstart/test/fixtures/booking` of that commit over
//! `fixtures/booking`, write its sha in `SOURCE`, and make this pass.

use std::{fs, path::PathBuf};

use panel_core::booking::{CAL_COM_HOSTS, check_config};
use serde_json::Value;

fn dir(sub: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/booking").join(sub)
}

fn read(path: &PathBuf) -> Value {
	let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
	serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

fn configs(sub: &str) -> Vec<(String, Value)> {
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
	assert!(source.starts_with("EV-Service-Arb/sites_tools"), "{source}");
	assert_eq!(configs("valid").len(), 14, "the vendored set");
	assert_eq!(configs("invalid").len(), 56, "the vendored set");
	let rules = read(&dir("rules.json"));
	assert_eq!(rules["calComHosts"], serde_json::json!(CAL_COM_HOSTS), "the Cal.com hosts are kitstart's default");
}

#[test]
fn every_valid_config_is_accepted_as_it_came() {
	for (name, config) in configs("valid") {
		match check_config(&config) {
			Ok(stored) => assert_eq!(stored, config, "{name}: stored otherwise than sent"),
			Err(problems) => panic!("{name} refused: {problems:?}"),
		}
	}
}

#[test]
fn every_invalid_config_is_refused() {
	for (name, config) in configs("invalid") {
		assert!(check_config(&config).is_err(), "{name} accepted");
	}
}
