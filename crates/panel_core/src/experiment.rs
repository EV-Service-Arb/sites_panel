//! A brand's experiments as configuration: what its landing declares at start
//! (`experiments.declared`), and what an admin lays over it (`experiment.configured`): a kill
//! switch, the weights, the holdout. The statistics are PostHog's; nothing here counts.
//!
//! The landing applies an override field only when it is valid against the variants in its own
//! code (`@evinvest/experiments` `applyOverrides`); [`fold`] judges by the same rules against the
//! latest declaration, so what the panel shows as effective is what the landings do — even when
//! a declaration changed the variants after an admin set the weights.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Timestamp;

use crate::{Invalid, ids::EventId};

/// What a landing's declaration may hold.
#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
	/// Experiments one declaration may hold.
	pub max_experiments: usize,
	/// Variants one experiment may have, two at least.
	pub max_variants: usize,
	/// The longest summary, in characters, once trimmed.
	pub max_summary: usize,
	/// An experiment's key.
	pub key: &'static str,
	/// A variant's name, what `@evinvest/experiments` takes.
	pub variant: &'static str,
}

pub const LIMITS: Limits = Limits {
	max_experiments: 64,
	max_variants: 32,
	max_summary: 200,
	key: "^[a-z0-9_]{1,64}$",
	variant: "^[a-z0-9_-]{1,32}$",
};
const MAX_VARIANTS: usize = LIMITS.max_variants;

/// What a landing says of one experiment in its code.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
	pub key: String,
	/// At least two, unique; the first is the control.
	pub variants: Vec<String>,
	pub weights: Vec<f64>,
	pub enabled: bool,
	pub holdout: Option<f64>,
	/// The hypothesis, in a line.
	pub summary: Option<String>,
}

// Every number in one is finite (checked on parse), so equality is reflexive.
impl Eq for Declaration {}

/// `[a-z0-9_]{1,64}`.
pub fn is_key(s: &str) -> bool {
	(1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A variant's name: `[a-z0-9_-]{1,32}`, what `@evinvest/experiments` takes.
fn is_variant(s: &str) -> bool {
	(1..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Weights a landing takes for `variants` variants (`@evinvest/experiments`' `validWeights`,
/// `contracts/vectors/overrides.json`): as many, each a finite number ≥ 0, and their sum > 0.
pub fn weights_ok(weights: &[f64], variants: usize) -> bool {
	weights.len() == variants && weights.iter().all(|w| w.is_finite() && *w >= 0.0) && weights.iter().sum::<f64>() > 0.0
}

/// A holdout a landing takes (`validHoldout`): in [0, 1).
pub fn holdout_ok(h: f64) -> bool {
	h.is_finite() && (0.0..1.0).contains(&h)
}

impl Declaration {
	pub fn parse(key: &str, variants: Vec<String>, weights: Vec<f64>, enabled: bool, holdout: Option<f64>, summary: Option<String>) -> Result<Self, Invalid> {
		let at = |what: &str| Invalid::new(format!("properties.experiments[{key}].{what}"));
		if !is_key(key) {
			return Err(Invalid::new("properties.experiments[].key is not 1–64 of [a-z0-9_]"));
		}
		if !(2..=MAX_VARIANTS).contains(&variants.len()) {
			return Err(at(&format!("variants are 2 to {MAX_VARIANTS}")));
		}
		if !variants.iter().all(|v| is_variant(v)) {
			return Err(at("variants are 1–32 of [a-z0-9_-]"));
		}
		if variants.iter().collect::<BTreeSet<_>>().len() != variants.len() {
			return Err(at("variants are not unique"));
		}
		if !weights_ok(&weights, variants.len()) {
			return Err(at("weights are one per variant, each ≥ 0, their sum > 0"));
		}
		if holdout.is_some_and(|h| !holdout_ok(h)) {
			return Err(at("holdout is not in [0, 1)"));
		}
		let summary = summary.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
		if summary.as_ref().is_some_and(|s| s.chars().count() > LIMITS.max_summary) {
			return Err(at(&format!("summary is at most {} characters", LIMITS.max_summary)));
		}
		Ok(Self {
			key: key.to_owned(),
			variants,
			weights,
			enabled,
			holdout,
			summary,
		})
	}
}

/// A whole declaration: at most `max_experiments`, each key once.
pub fn check_declared(experiments: &[Declaration]) -> Result<(), Invalid> {
	if experiments.len() > LIMITS.max_experiments {
		return Err(Invalid::new(format!("properties.experiments holds more than {}", LIMITS.max_experiments)));
	}
	if experiments.iter().map(|e| &e.key).collect::<BTreeSet<_>>().len() != experiments.len() {
		return Err(Invalid::new("properties.experiments names a key twice"));
	}
	Ok(())
}

/// One field of an admin's change: left as it is, set, or put back to the declaration.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Field<T> {
	#[default]
	Keep,
	Set(T),
	Reset,
}

impl<T: Clone> Field<T> {
	fn apply(&self, current: &mut Option<T>) {
		match self {
			Self::Keep => {}
			Self::Set(v) => *current = Some(v.clone()),
			Self::Reset => *current = None,
		}
	}
}

/// What an admin changed of one experiment (`experiment.configured`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
	pub key: String,
	pub enabled: Field<bool>,
	pub weights: Field<Vec<f64>>,
	pub holdout: Field<f64>,
}

// As for `Declaration`: a set weight or holdout is finite.
impl Eq for Patch {}

/// The fields a patch may put back, as the event names them.
pub const RESETTABLE: [&str; 3] = ["enabled", "weights", "holdout"];

impl Patch {
	/// Checked on its own: a set field well-formed (weights 2–32 of them, a holdout in [0, 1)),
	/// no field both set and put back. Whether the weights fit the variants is the declaration's
	/// question ([`State::check_patch`]).
	pub fn parse(key: &str, enabled: Option<bool>, weights: Vec<f64>, holdout: Option<f64>, reset: &[String]) -> Result<Self, Invalid> {
		if !is_key(key) {
			return Err(Invalid::new("properties.key is not 1–64 of [a-z0-9_]"));
		}
		if let Some(other) = reset.iter().find(|r| !RESETTABLE.contains(&r.as_str())) {
			return Err(Invalid::new(format!("properties.reset names {other:?}, not one of enabled, weights, holdout")));
		}
		if !weights.is_empty() && !weights_ok(&weights, weights.len().clamp(2, MAX_VARIANTS)) {
			return Err(Invalid::new("properties.weights are 2–32 numbers, each ≥ 0, their sum > 0"));
		}
		if holdout.is_some_and(|h| !holdout_ok(h)) {
			return Err(Invalid::new("properties.holdout is not in [0, 1)"));
		}
		Ok(Self {
			key: key.to_owned(),
			enabled: field(enabled, "enabled", reset)?,
			weights: field((!weights.is_empty()).then_some(weights), "weights", reset)?,
			holdout: field(holdout, "holdout", reset)?,
		})
	}

	pub fn is_empty(&self) -> bool {
		self.enabled == Field::Keep && self.weights == Field::Keep && self.holdout == Field::Keep
	}
}

/// One field of a patch from what the event says of it.
fn field<T>(set: Option<T>, name: &str, reset: &[String]) -> Result<Field<T>, Invalid> {
	match (set, reset.iter().any(|r| r == name)) {
		(Some(_), true) => Err(Invalid::new(format!("properties.{name} is both set and reset"))),
		(Some(v), false) => Ok(Field::Set(v)),
		(None, true) => Ok(Field::Reset),
		(None, false) => Ok(Field::Keep),
	}
}

/// The longest label of who changed an experiment.
pub const MAX_LABEL: usize = 320;

/// Who changed an experiment, as the screens show them: the admin's email, or their concierge id
/// when concierge gave none (`place::Editor::label`, as a place's or a price list's history
/// names them). An employee's, not a customer's: not PII. One line, 1–320 characters.
pub fn label(raw: &str) -> Result<String, Invalid> {
	let l = raw.trim();
	if (1..=MAX_LABEL).contains(&l.chars().count()) && !l.chars().any(char::is_control) {
		Ok(l.to_owned())
	} else {
		Err(Invalid::new(format!("properties.by is one line of 1–{MAX_LABEL} characters")))
	}
}

/// What an admin has set over a declaration; `None` fields follow it.
#[derive(Clone, Debug, PartialEq)]
pub struct Override {
	pub enabled: Option<bool>,
	pub weights: Option<Vec<f64>>,
	pub holdout: Option<f64>,
	/// Who made the last change ([`label`]).
	pub changed_by: String,
	pub changed_at: Timestamp,
}

impl Override {
	fn is_empty(&self) -> bool {
		self.enabled.is_none() && self.weights.is_none() && self.holdout.is_none()
	}
}

/// What a landing runs: the declaration with every valid override field laid over it.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
	pub enabled: bool,
	pub weights: Vec<f64>,
	pub holdout: Option<f64>,
}

/// One experiment of a brand, folded from its declarations and changes.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
	/// As the latest declaration naming it said it.
	pub declared: Declaration,
	/// When that declaration happened.
	pub declared_at: Timestamp,
	/// The first declaration naming it.
	pub first_declared_at: Timestamp,
	/// `None` when nothing is set, or everything was put back.
	pub over: Option<Override>,
	/// The last time the weights a landing runs changed, by a declaration or an admin; `None`
	/// while they are the first declared.
	pub weights_changed_at: Option<Timestamp>,
	/// Not in the brand's latest declaration: shown, not changed.
	pub retired: bool,
}

impl State {
	/// The fields of the override a landing takes, by the rules it applies them by.
	pub fn valid_override(&self) -> (Option<bool>, Option<&[f64]>, Option<f64>) {
		let Some(o) = &self.over else { return (None, None, None) };
		let weights = o.weights.as_deref().filter(|w| weights_ok(w, self.declared.variants.len()));
		(o.enabled, weights, o.holdout.filter(|h| holdout_ok(*h)))
	}

	pub fn effective(&self) -> Effective {
		let (enabled, weights, holdout) = self.valid_override();
		Effective {
			enabled: enabled.unwrap_or(self.declared.enabled),
			weights: weights.map_or_else(|| self.declared.weights.clone(), <[f64]>::to_vec),
			holdout: holdout.or(self.declared.holdout),
		}
	}

	/// Whether an admin's change fits: the experiment is current, and set weights are one per
	/// declared variant.
	pub fn check_patch(&self, patch: &Patch) -> Result<(), Invalid> {
		if let Field::Set(w) = &patch.weights
			&& !weights_ok(w, self.declared.variants.len())
		{
			return Err(Invalid::new(format!(
				"weights are {} numbers, one per variant, each ≥ 0, their sum > 0",
				self.declared.variants.len()
			)));
		}
		Ok(())
	}

	/// The override after `patch`; `None` when nothing is left set.
	pub fn patched(&self, patch: &Patch, by: &str, at: Timestamp) -> Option<Override> {
		let mut o = self.over.clone().unwrap_or(Override {
			enabled: None,
			weights: None,
			holdout: None,
			changed_by: by.to_owned(),
			changed_at: at,
		});
		patch.enabled.apply(&mut o.enabled);
		patch.weights.apply(&mut o.weights);
		patch.holdout.apply(&mut o.holdout);
		o.changed_by = by.to_owned();
		o.changed_at = at;
		(!o.is_empty()).then_some(o)
	}

	/// Whether `patch` would change what is set.
	pub fn changes(&self, patch: &Patch) -> bool {
		let fields = |o: &Option<Override>| o.as_ref().map(|o| (o.enabled, o.weights.clone(), o.holdout));
		fields(&self.over) != fields(&self.patched(patch, "", Timestamp::UNIX_EPOCH))
	}
}

/// One event about a brand's experiments.
#[derive(Clone, Debug, PartialEq)]
pub enum Happened {
	Declared(Vec<Declaration>),
	/// `by`: the event's source id.
	Configured {
		patch: Patch,
		by: String,
	},
}

/// A brand's experiments from all its events about them, by key: the same answer in whatever
/// order they are given (sorted by `(occurred_at, id)` here).
pub fn fold(events: &[(Timestamp, EventId, Happened)]) -> BTreeMap<String, State> {
	let mut ordered: Vec<&(Timestamp, EventId, Happened)> = events.iter().collect();
	ordered.sort_by_key(|(at, id, _)| (*at, id.raw()));
	let mut states: BTreeMap<String, State> = BTreeMap::new();
	for (at, _, happened) in ordered {
		let before: BTreeMap<String, Vec<f64>> = states.iter().map(|(k, s)| (k.clone(), s.effective().weights)).collect();
		match happened {
			Happened::Declared(declared) => {
				let current: BTreeSet<&str> = declared.iter().map(|d| d.key.as_str()).collect();
				for s in states.values_mut() {
					s.retired = !current.contains(s.declared.key.as_str());
				}
				for d in declared {
					states
						.entry(d.key.clone())
						.and_modify(|s| {
							s.declared = d.clone();
							s.declared_at = *at;
						})
						.or_insert_with(|| State {
							declared: d.clone(),
							declared_at: *at,
							first_declared_at: *at,
							over: None,
							weights_changed_at: None,
							retired: false,
						});
				}
			}
			// A change of an experiment never declared names nothing: the API refuses it, and one
			// that slipped in is left out.
			Happened::Configured { patch, by } =>
				if let Some(s) = states.get_mut(&patch.key) {
					s.over = s.patched(patch, by, *at);
				},
		}
		for (k, s) in &mut states {
			if before.get(k).is_some_and(|w| *w != s.effective().weights) {
				s.weights_changed_at = Some(*at);
			}
		}
	}
	states
}

#[cfg(test)]
mod tests {
	use uuid::Uuid;

	use super::*;

	fn at(minutes: i64) -> Timestamp {
		"2026-10-04T10:00:00Z".parse::<Timestamp>().unwrap() + jiff::SignedDuration::from_mins(minutes)
	}

	fn decl(key: &str, variants: &[&str], weights: &[f64]) -> Declaration {
		Declaration::parse(key, variants.iter().map(|v| (*v).to_owned()).collect(), weights.to_vec(), true, None, None).unwrap()
	}

	fn ev(minutes: i64, h: Happened) -> (Timestamp, EventId, Happened) {
		(at(minutes), EventId::from_raw(Uuid::now_v7()), h)
	}

	fn set_weights(key: &str, w: &[f64]) -> Happened {
		Happened::Configured {
			patch: Patch::parse(key, None, w.to_vec(), None, &[]).unwrap(),
			by: "admin-1".into(),
		}
	}

	#[test]
	fn declarations_are_checked() {
		let v = |xs: &[&str]| xs.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
		assert!(Declaration::parse("lead_layout", v(&["a", "b"]), vec![1.0, 0.0], false, Some(0.0), Some(" one line ".into())).is_ok());
		assert!(
			Declaration::parse("k", v(&["_control", "-b", &"c".repeat(32)]), vec![1.0, 1.0, 1.0], true, None, Some("x".repeat(200))).is_ok(),
			"no stricter than the landing's own check"
		);
		for (key, variants, weights, holdout, summary, want) in [
			("Lead", v(&["a", "b"]), vec![1.0, 1.0], None, None, "key"),
			("k", v(&["a"]), vec![1.0], None, None, "variants are 2"),
			("k", v(&["a", "a"]), vec![1.0, 1.0], None, None, "not unique"),
			("k", v(&["a", "B"]), vec![1.0, 1.0], None, None, "variants are 1–32"),
			("k", v(&["a", &"b".repeat(33)]), vec![1.0, 1.0], None, None, "variants are 1–32"),
			("k", v(&["a", "b"]), vec![1.0], None, None, "weights"),
			("k", v(&["a", "b"]), vec![0.0, 0.0], None, None, "weights"),
			("k", v(&["a", "b"]), vec![-1.0, 2.0], None, None, "weights"),
			("k", v(&["a", "b"]), vec![f64::NAN, 1.0], None, None, "weights"),
			("k", v(&["a", "b"]), vec![1.0, 1.0], Some(1.0), None, "holdout"),
			("k", v(&["a", "b"]), vec![1.0, 1.0], None, Some("x".repeat(201)), "summary"),
		] {
			let e = Declaration::parse(key, variants, weights, true, holdout, summary).unwrap_err();
			assert!(e.0.contains(want), "{key}: {e}");
		}
		assert!(check_declared(&[decl("a", &["x", "y"], &[1.0, 1.0]), decl("a", &["x", "y"], &[1.0, 1.0])]).is_err());
	}

	#[test]
	fn labels() {
		assert_eq!(label(" ops@evinvest.ltd ").unwrap(), "ops@evinvest.ltd");
		assert!(label("").is_err());
		assert!(label("a\nb").is_err());
		assert!(label(&"x".repeat(321)).is_err());
	}

	#[test]
	fn patches_are_checked() {
		assert_eq!(Patch::parse("k", None, vec![], Some(0.0), &[]).unwrap().holdout, Field::Set(0.0), "a holdout of 0 is valid");
		assert_eq!(Patch::parse("k", None, vec![], None, &["weights".into()]).unwrap().weights, Field::Reset);
		assert!(Patch::parse("k", Some(true), vec![], None, &["enabled".into()]).is_err());
		assert!(Patch::parse("k", None, vec![], None, &["variants".into()]).is_err());
		assert!(Patch::parse("k", None, vec![0.0, 0.0], None, &[]).is_err());
		assert!(Patch::parse("k", None, vec![], Some(-0.1), &[]).is_err());
		assert!(Patch::parse("k", None, vec![], None, &[]).unwrap().is_empty());
	}

	#[test]
	fn an_override_lays_over_the_declaration_while_it_fits() {
		let s = fold(&[ev(0, Happened::Declared(vec![decl("hero", &["a", "b"], &[1.0, 1.0])])), ev(5, set_weights("hero", &[3.0, 1.0]))]);
		let hero = &s["hero"];
		assert_eq!(hero.effective().weights, [3.0, 1.0]);
		assert_eq!(hero.weights_changed_at, Some(at(5)));
		assert_eq!(hero.first_declared_at, at(0));

		// The landing adds a variant: the admin's two weights no longer fit, and are ignored.
		let s = fold(&[
			ev(0, Happened::Declared(vec![decl("hero", &["a", "b"], &[1.0, 1.0])])),
			ev(5, set_weights("hero", &[3.0, 1.0])),
			ev(9, Happened::Declared(vec![decl("hero", &["a", "b", "c"], &[1.0, 1.0, 1.0])])),
		]);
		assert_eq!(s["hero"].effective().weights, [1.0, 1.0, 1.0]);
		assert_eq!(s["hero"].valid_override().1, None);
		assert_eq!(s["hero"].weights_changed_at, Some(at(9)));
	}

	#[test]
	fn the_order_given_does_not_matter_and_gone_is_retired() {
		let mut events = vec![
			ev(0, Happened::Declared(vec![decl("hero", &["a", "b"], &[1.0, 1.0]), decl("cta", &["a", "b"], &[1.0, 1.0])])),
			ev(
				3,
				Happened::Configured {
					patch: Patch::parse("cta", Some(false), vec![], None, &[]).unwrap(),
					by: "admin-1".into(),
				},
			),
			ev(6, Happened::Declared(vec![decl("hero", &["a", "b"], &[1.0, 1.0])])),
		];
		let forward = fold(&events);
		events.reverse();
		assert_eq!(fold(&events), forward);
		assert!(forward["cta"].retired && !forward["hero"].retired);
		assert!(!forward["cta"].effective().enabled);
		assert_eq!(forward["hero"].weights_changed_at, None, "the same weights declared again");
	}

	#[test]
	fn putting_everything_back_leaves_no_override() {
		let s = fold(&[ev(0, Happened::Declared(vec![decl("hero", &["a", "b"], &[1.0, 1.0])])), ev(1, set_weights("hero", &[2.0, 1.0]))]);
		let back = Patch::parse("hero", None, vec![], None, &["weights".into()]).unwrap();
		assert!(s["hero"].changes(&back));
		assert_eq!(s["hero"].patched(&back, "admin-1", at(2)), None);
		let same = Patch::parse("hero", None, vec![2.0, 1.0], None, &[]).unwrap();
		assert!(!s["hero"].changes(&same));
		assert!(s["hero"].check_patch(&Patch::parse("hero", None, vec![1.0, 1.0, 1.0], None, &[]).unwrap()).is_err());
	}
}
