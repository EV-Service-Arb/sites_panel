//! A brand's price list as data (kitstart's `PricingModel`, EV-Service-Arb/sites_tools `ts/kitstart/src/core/pricing`): what
//! an `estimate` need costs for a set of enum answers, and what a `fixed` need costs outright.
//! The panel stores it, serves it to the sites, and prices the editor's preview with its own
//! [`price_of`] — so a preview is what the site shows only if the two agree to the cent.
//!
//! What holds them together is kitstart's fixtures, vendored by commit in
//! `tests/fixtures/pricing/` (their README is the normative text): every valid model accepted,
//! every invalid one refused whole, every case priced to the cent. The checks here follow
//! kitstart's `validate.ts` rule for rule and say where, with kitstart's paths
//! (`model.needs.standard.inputs[2]`); the arithmetic follows `price.ts` step for step, on
//! integers only.
//!
//! One difference that cannot be helped: JSON objects are read into sorted maps here, in
//! insertion order there, so when a model has several problems the first one named may differ
//! — never whether there is one.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::Invalid;

/// The model's format; a model of another is refused whole.
pub const FORMAT: i64 = 1;
/// Euros, all taxes included: the only currency a model may be in today.
pub const CURRENCY: &str = "EUR";

/// The locales a brand's sites speak until told otherwise: both landings are French and English.
pub const DEFAULT_LOCALES: [&str; 2] = ["fr", "en"];
/// More than any site will speak; a bound, not a plan.
pub const MAX_LOCALES: usize = 8;

/// kitstart's `PRICING_LIMITS`.
pub mod limits {
	pub const MAX_LABEL: usize = 120;
	pub const MAX_INPUTS: usize = 32;
	pub const MAX_OPTIONS: usize = 32;
	pub const MAX_NEEDS: usize = 64;
	/// Per need: what a lead's `estimate_inputs` may carry.
	pub const MAX_NEED_INPUTS: usize = 12;
	pub const MAX_PRICE_CENTS: i64 = 100_000_000;
	/// ×10 at most.
	pub const MAX_MULTIPLY_BP: i64 = 100_000;
	pub const MAX_DISCOUNT_BP: i64 = 10_000;
}

use limits::*;

/// 10 000 basis points: ×1.
const BP: i64 = 10_000;

/// A label per locale (`{fr: "Studio", en: "Studio"}`); the page picks its own.
pub type Labels = BTreeMap<String, String>;

/// What an input's options do to the price, and when: every `add` first, then every
/// `multiply`, then every `discount`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputKind {
	/// `addCents`, added to the base.
	Add,
	/// `multiplyBp`: the subtotal times this, in basis points (11 000 is ×1.1).
	Multiply,
	/// `discountBp`: taken off the subtotal, in basis points (1 000 is 10 % off).
	Discount,
}

impl InputKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Add => "add",
			Self::Multiply => "multiply",
			Self::Discount => "discount",
		}
	}

	/// The field an option of this kind carries its effect in.
	pub fn effect_field(self) -> &'static str {
		match self {
			Self::Add => "addCents",
			Self::Multiply => "multiplyBp",
			Self::Discount => "discountBp",
		}
	}

	fn effect_max(self) -> i64 {
		match self {
			Self::Add => MAX_PRICE_CENTS,
			Self::Multiply => MAX_MULTIPLY_BP,
			Self::Discount => MAX_DISCOUNT_BP,
		}
	}

	fn parse(raw: &Value) -> Option<Self> {
		match raw.as_str()? {
			"add" => Some(Self::Add),
			"multiply" => Some(Self::Multiply),
			"discount" => Some(Self::Discount),
			_ => None,
		}
	}
}

/// One choice of an input: its slug, its words, and its effect (in the unit its input's kind
/// says).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingOption {
	pub id: String,
	pub labels: Labels,
	pub effect: i64,
}

/// An enum the visitor answers: bedrooms, a surface band, a zone, a frequency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingInput {
	pub id: String,
	pub kind: InputKind,
	pub labels: Labels,
	pub options: Vec<PricingOption>,
}

/// How one need is priced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NeedPricing {
	/// A base and the inputs the visitor answers, in the order the form asks them.
	Estimate { base_cents: i64, inputs: Vec<String> },
	/// One price, exactly: neither rounded nor raised to the minimum.
	Fixed { cents: i64 },
}

/// A checked model. Only [`PricingModel::parse`] makes one, so every value in it is within
/// the limits — which is what keeps [`price_of`] within an `i64` with room to spare.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingModel {
	/// `YYYY-MM-DD`, a day the calendar has: when the prices hold from.
	pub valid_from: String,
	pub round_to_cents: i64,
	pub minimum_cents: i64,
	pub inputs: Vec<PricingInput>,
	/// By the brand's need slug; a need not here is a quote.
	pub needs: BTreeMap<String, NeedPricing>,
}

/// The answers to an estimate's inputs: input id → option id.
pub type PricingInputs = BTreeMap<String, String>;

/// One thing wrong with a model: where (kitstart's path, `model.inputs[0].options[2].addCents`)
/// and why (kitstart's words).
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{path}: {why}")]
pub struct Problem {
	pub path: String,
	pub why: String,
}

/// Collects problems at a path, as kitstart's `Check` does.
#[derive(Default)]
struct Check {
	problems: Vec<Problem>,
}

/// kitstart's `isObject`: an object, not an array or null.
fn object(v: &Value) -> Option<&Map<String, Value>> {
	v.as_object()
}

/// `^[a-z0-9_-]{1,40}$`.
pub fn is_slug(s: &str) -> bool {
	(1..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// `^[a-z]{2}(-[A-Z]{2})?$`: `fr`, `en`, `fr-FR`.
pub fn is_locale(s: &str) -> bool {
	let b = s.as_bytes();
	let lang = |b: &[u8]| b.iter().all(u8::is_ascii_lowercase);
	match b.len() {
		2 => lang(b),
		5 => lang(&b[..2]) && b[2] == b'-' && b[3..].iter().all(u8::is_ascii_uppercase),
		_ => false,
	}
}

/// `YYYY-MM-DD` of ASCII digits, and a day the (proleptic Gregorian) calendar has — what
/// kitstart's `isDate` accepts.
pub fn is_date(s: &str) -> bool {
	let b = s.as_bytes();
	let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
	if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(0..4) || !digits(5..7) || !digits(8..10) {
		return false;
	}
	let num = |r: std::ops::Range<usize>| b[r].iter().fold(0u32, |n, d| n * 10 + u32::from(d - b'0'));
	let (year, month, day) = (num(0..4), num(5..7), num(8..10));
	let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
	let days = match month {
		1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
		4 | 6 | 9 | 11 => 30,
		2 if leap => 29,
		2 => 28,
		_ => return false,
	};
	(1..=days).contains(&day)
}

/// JavaScript's `Number.isSafeInteger`, on a JSON number: `100.0` and `1e2` are the integer
/// 100 there, as they are here.
fn safe_integer(v: &Value) -> Option<i64> {
	const MAX_SAFE: i64 = (1 << 53) - 1;
	let n = match v {
		Value::Number(n) => n,
		_ => return None,
	};
	if let Some(i) = n.as_i64() {
		return (-MAX_SAFE..=MAX_SAFE).contains(&i).then_some(i);
	}
	if n.is_u64() {
		return None;
	}
	let f = n.as_f64()?;
	// A float within ±2^53 with no fraction is exactly an integer, so the cast is exact.
	(f.fract() == 0.0 && f.abs() <= MAX_SAFE as f64).then_some(f as i64)
}

/// `v` as text the way JavaScript counts it: in UTF-16 code units.
fn js_len(s: &str) -> usize {
	s.encode_utf16().count()
}

impl Check {
	fn fail(&mut self, path: impl Into<String>, why: impl Into<String>) {
		self.problems.push(Problem { path: path.into(), why: why.into() });
	}

	/// Only these keys, all of them required.
	fn keys(&mut self, path: &str, v: &Map<String, Value>, required: &[&str]) {
		for key in required {
			if !v.contains_key(*key) {
				self.fail(format!("{path}.{key}"), "missing");
			}
		}
		for key in v.keys() {
			if !required.contains(&key.as_str()) {
				self.fail(format!("{path}.{key}"), "unknown field");
			}
		}
	}

	fn int(&mut self, path: &str, v: Option<&Value>, min: i64, max: i64) -> i64 {
		match v.and_then(safe_integer) {
			Some(n) if (min..=max).contains(&n) => n,
			_ => {
				self.fail(path, format!("an integer from {min} to {max}"));
				min
			}
		}
	}

	fn slug(&mut self, path: &str, v: Option<&Value>) -> String {
		match v.and_then(Value::as_str) {
			Some(s) if is_slug(s) => s.to_owned(),
			_ => {
				self.fail(path, "a slug, [a-z0-9_-]{1,40}");
				String::new()
			}
		}
	}

	fn labels(&mut self, path: &str, v: Option<&Value>) -> Labels {
		let Some(m) = v.and_then(object).filter(|m| !m.is_empty()) else {
			self.fail(path, "an object of labels by locale, at least one");
			return Labels::new();
		};
		let mut out = Labels::new();
		for (locale, text) in m {
			if !is_locale(locale) {
				self.fail(format!("{path}.{locale}"), "not a locale (fr, en, fr-FR)");
				continue;
			}
			match text.as_str() {
				Some(t) if !t.trim().is_empty() && js_len(t) <= MAX_LABEL => {
					out.insert(locale.clone(), t.to_owned());
				}
				_ => self.fail(format!("{path}.{locale}"), format!("a label of 1 to {MAX_LABEL} characters")),
			}
		}
		out
	}

	fn list<'a>(&mut self, path: &str, v: Option<&'a Value>, min: usize, max: usize) -> &'a [Value] {
		match v.and_then(Value::as_array) {
			Some(a) if (min..=max).contains(&a.len()) => a,
			_ => {
				self.fail(path, format!("a list of {min} to {max}"));
				&[]
			}
		}
	}

	fn unique<'a>(&mut self, path: &str, ids: impl IntoIterator<Item = &'a str>) {
		let mut seen = std::collections::BTreeSet::new();
		if let Some(dup) = ids.into_iter().filter(|id| !id.is_empty()).find(|id| !seen.insert(*id)) {
			self.fail(path, format!("\"{dup}\" is listed twice"));
		}
	}

	fn input(&mut self, path: &str, v: &Value) -> Option<PricingInput> {
		let Some(v) = object(v) else {
			self.fail(path, "an object");
			return None;
		};
		self.keys(path, v, &["id", "kind", "labels", "options"]);
		let id = self.slug(&format!("{path}.id"), v.get("id"));
		let labels = self.labels(&format!("{path}.labels"), v.get("labels"));
		let Some(kind) = v.get("kind").and_then(InputKind::parse) else {
			self.fail(format!("{path}.kind"), "add, multiply or discount");
			return None;
		};
		let effect = kind.effect_field();
		let mut options = Vec::new();
		for (i, o) in self.list(&format!("{path}.options"), v.get("options"), 1, MAX_OPTIONS).iter().enumerate() {
			let at = format!("{path}.options[{i}]");
			let Some(o) = object(o) else {
				self.fail(&at, "an object");
				options.push(PricingOption {
					id: String::new(),
					labels: Labels::new(),
					effect: 0,
				});
				continue;
			};
			self.keys(&at, o, &["id", "labels", effect]);
			options.push(PricingOption {
				id: self.slug(&format!("{at}.id"), o.get("id")),
				labels: self.labels(&format!("{at}.labels"), o.get("labels")),
				effect: self.int(&format!("{at}.{effect}"), o.get(effect), 0, kind.effect_max()),
			});
		}
		self.unique(&format!("{path}.options"), options.iter().map(|o| o.id.as_str()));
		Some(PricingInput { id, kind, labels, options })
	}

	fn need(&mut self, path: &str, v: &Value, inputs: &[PricingInput]) -> Option<NeedPricing> {
		let Some(v) = object(v) else {
			self.fail(path, "an object");
			return None;
		};
		match v.get("kind").and_then(Value::as_str) {
			Some("fixed") => {
				self.keys(path, v, &["kind", "cents"]);
				return Some(NeedPricing::Fixed {
					cents: self.int(&format!("{path}.cents"), v.get("cents"), 0, MAX_PRICE_CENTS),
				});
			}
			Some("estimate") => {}
			_ => {
				self.fail(format!("{path}.kind"), "estimate or fixed");
				return None;
			}
		}
		self.keys(path, v, &["kind", "baseCents", "inputs"]);
		let base_cents = self.int(&format!("{path}.baseCents"), v.get("baseCents"), 0, MAX_PRICE_CENTS);
		let listed = self.list(&format!("{path}.inputs"), v.get("inputs"), 0, MAX_NEED_INPUTS);
		let ids: Vec<String> = listed.iter().enumerate().map(|(i, id)| self.slug(&format!("{path}.inputs[{i}]"), Some(id))).collect();
		self.unique(&format!("{path}.inputs"), ids.iter().map(String::as_str));
		for (i, id) in ids.iter().enumerate() {
			if !id.is_empty() && !inputs.iter().any(|input| input.id == *id) {
				self.fail(format!("{path}.inputs[{i}]"), format!("no input \"{id}\""));
			}
		}
		Some(NeedPricing::Estimate { base_cents, inputs: ids })
	}

	/// The dearest answer to every input — each effect only raises the price with its value,
	/// and rounding half up is monotonic — walked step by step: if no step of it passes the
	/// cap, no combination does.
	fn dearest(&mut self, path: &str, base_cents: i64, asked: &[String], inputs: &[PricingInput]) {
		let asked: Vec<&PricingInput> = asked.iter().filter_map(|id| inputs.iter().find(|i| i.id == *id)).collect();
		let max = |i: &PricingInput| match i.kind {
			InputKind::Add | InputKind::Multiply => i.options.iter().map(|o| o.effect).max().unwrap_or(0),
			InputKind::Discount => 0,
		};
		let mut total = base_cents;
		let mut over = total > MAX_PRICE_CENTS;
		for i in asked.iter().filter(|i| i.kind == InputKind::Add) {
			total += max(i);
			over |= total > MAX_PRICE_CENTS;
		}
		for i in asked.iter().filter(|i| i.kind == InputKind::Multiply) {
			total = mul_bp(total, max(i));
			over |= total > MAX_PRICE_CENTS;
		}
		if over {
			self.fail(path, format!("its dearest combination passes {MAX_PRICE_CENTS} cents"));
		}
	}
}

impl PricingModel {
	/// The model `value` is, or every reason it is not one (kitstart's `pricingProblems`):
	/// taken whole or not at all, unknown fields refused rather than ignored.
	pub fn parse(value: &Value) -> Result<Self, Vec<Problem>> {
		let mut c = Check::default();
		let Some(v) = object(value) else {
			c.fail("model", "an object");
			return Err(c.problems);
		};
		c.keys("model", v, &["format", "currency", "validFrom", "roundToCents", "minimumCents", "inputs", "needs"]);
		if v.get("format").and_then(safe_integer) != Some(FORMAT) {
			c.fail("model.format", FORMAT.to_string());
		}
		if v.get("currency").and_then(Value::as_str) != Some(CURRENCY) {
			c.fail("model.currency", CURRENCY);
		}
		let valid_from = v.get("validFrom").and_then(Value::as_str).filter(|d| is_date(d));
		if valid_from.is_none() {
			c.fail("model.validFrom", "a date, YYYY-MM-DD");
		}
		let round_to_cents = c.int("model.roundToCents", v.get("roundToCents"), 1, MAX_PRICE_CENTS);
		let minimum_cents = c.int("model.minimumCents", v.get("minimumCents"), 0, MAX_PRICE_CENTS);
		let listed = c.list("model.inputs", v.get("inputs"), 0, MAX_INPUTS);
		let inputs: Vec<PricingInput> = listed.iter().enumerate().filter_map(|(i, input)| c.input(&format!("model.inputs[{i}]"), input)).collect();
		c.unique("model.inputs", inputs.iter().map(|i| i.id.as_str()));
		let mut needs = BTreeMap::new();
		match v.get("needs").and_then(object) {
			None => c.fail("model.needs", "an object by need slug"),
			Some(entries) => {
				if entries.len() > MAX_NEEDS {
					c.fail("model.needs", format!("at most {MAX_NEEDS}"));
				}
				for (slug, need) in entries {
					let path = format!("model.needs.{slug}");
					if !is_slug(slug) {
						c.fail(&path, "the need must be a slug, [a-z0-9_-]{1,40}");
					}
					if let Some(pricing) = c.need(&path, need, &inputs) {
						needs.insert(slug.clone(), pricing);
					}
				}
			}
		}
		if !c.problems.is_empty() {
			return Err(c.problems);
		}
		for (slug, pricing) in &needs {
			if let NeedPricing::Estimate { base_cents, inputs: asked } = pricing {
				c.dearest(&format!("model.needs.{slug}"), *base_cents, asked, &inputs);
			}
		}
		match valid_from {
			Some(valid_from) if c.problems.is_empty() => Ok(Self {
				valid_from: valid_from.to_owned(),
				round_to_cents,
				minimum_cents,
				inputs,
				needs,
			}),
			_ => Err(c.problems),
		}
	}

	/// A model a site can show (kitstart's `pricingProblemsFor`): valid, and every input and
	/// option labelled in each of `locales` — a site refuses a model with an option it has no
	/// words for.
	pub fn parse_for(value: &Value, locales: &[String]) -> Result<Self, Vec<Problem>> {
		let model = Self::parse(value)?;
		let missing = model.unlabelled(locales);
		if missing.is_empty() { Ok(model) } else { Err(missing) }
	}

	/// Every label `locales` lacks, by kitstart's path.
	pub fn unlabelled(&self, locales: &[String]) -> Vec<Problem> {
		let mut out = Vec::new();
		let mut labelled = |path: String, labels: &Labels| {
			for locale in locales {
				if !labels.contains_key(locale) {
					out.push(Problem {
						path: format!("{path}.labels"),
						why: format!("no \"{locale}\" label"),
					});
				}
			}
		};
		for (i, input) in self.inputs.iter().enumerate() {
			labelled(format!("model.inputs[{i}]"), &input.labels);
			for (j, o) in input.options.iter().enumerate() {
				labelled(format!("model.inputs[{i}].options[{j}]"), &o.labels);
			}
		}
		out
	}

	/// The model in kitstart's wire shape; [`PricingModel::parse`] reads it back unchanged.
	pub fn to_json(&self) -> Value {
		let inputs: Vec<Value> = self
			.inputs
			.iter()
			.map(|i| {
				let options: Vec<Value> = i
					.options
					.iter()
					.map(|o| {
						let mut option = json!({ "id": o.id, "labels": o.labels });
						option[i.kind.effect_field()] = json!(o.effect);
						option
					})
					.collect();
				json!({ "id": i.id, "kind": i.kind.as_str(), "labels": i.labels, "options": options })
			})
			.collect();
		let needs: Map<String, Value> = self
			.needs
			.iter()
			.map(|(slug, n)| {
				let n = match n {
					NeedPricing::Estimate { base_cents, inputs } => json!({ "kind": "estimate", "baseCents": base_cents, "inputs": inputs }),
					NeedPricing::Fixed { cents } => json!({ "kind": "fixed", "cents": cents }),
				};
				(slug.clone(), n)
			})
			.collect();
		json!({
			"format": FORMAT,
			"currency": CURRENCY,
			"validFrom": self.valid_from,
			"roundToCents": self.round_to_cents,
			"minimumCents": self.minimum_cents,
			"inputs": inputs,
			"needs": needs,
		})
	}
}

/// `cents × bp / 10 000`, rounded half up to the cent: `floor((cents × bp + 5 000) / 10 000)`.
/// Every operand is non-negative in a checked model, so the floor is the integer division.
pub fn mul_bp(cents: i64, bp: i64) -> i64 {
	let n = i128::from(cents) * i128::from(bp) + i128::from(BP / 2);
	// A checked model keeps every step at or under MAX_PRICE_CENTS × 10, so this never
	// saturates; saturating instead of panicking keeps an unchecked caller's bug a wrong price.
	i64::try_from(n / i128::from(BP)).unwrap_or(i64::MAX)
}

/// Half up to a multiple of `step` (≥ 1).
pub fn round_to(cents: i64, step: i64) -> i64 {
	let rest = cents % step;
	if rest * 2 >= step { cents - rest + step } else { cents - rest }
}

/// What `need` costs for these answers, in cents — kitstart's `priceOf`, step for step:
///
/// 1. the need's base, plus every `add` input, in the need's input order;
/// 2. times every `multiply` input, in order, each product rounded half up to the cent at once;
/// 3. every `discount` input, in order, as a multiplication by `10 000 − discountBp`, rounded
///    the same way;
/// 4. the total rounded half up to a multiple of `roundToCents`;
/// 5. raised to `minimumCents`.
///
/// A `fixed` need is its price, as is. `None`: the need is not priced, or one of its inputs is
/// unanswered or answered with an option it does not have — never a guess. Answers to inputs
/// the need does not ask are ignored.
pub fn price_of(model: &PricingModel, need: &str, answers: &PricingInputs) -> Option<i64> {
	let (base_cents, asked) = match model.needs.get(need)? {
		NeedPricing::Fixed { cents } => return Some(*cents),
		NeedPricing::Estimate { base_cents, inputs } => (*base_cents, inputs),
	};
	let mut chosen = Vec::with_capacity(asked.len());
	for id in asked {
		let input = model.inputs.iter().find(|i| i.id == *id)?;
		let answer = answers.get(id)?;
		let option = input.options.iter().find(|o| o.id == *answer)?;
		chosen.push((input.kind, option.effect));
	}
	let of = |kind: InputKind| chosen.iter().filter(move |(k, _)| *k == kind).map(|(_, effect)| *effect);
	let mut total = base_cents;
	total += of(InputKind::Add).sum::<i64>();
	for bp in of(InputKind::Multiply) {
		total = mul_bp(total, bp);
	}
	for bp in of(InputKind::Discount) {
		total = mul_bp(total, BP - bp);
	}
	Some(round_to(total, model.round_to_cents).max(model.minimum_cents))
}

/// A brand's locales as given (`fr,en`): 1 to [`MAX_LOCALES`], each a locale, each once — the
/// languages every label of its model must be in.
pub fn parse_locales<'a>(raw: impl IntoIterator<Item = &'a str>) -> Result<Vec<String>, Invalid> {
	let mut out: Vec<String> = Vec::new();
	for locale in raw {
		let locale = locale.trim();
		if !is_locale(locale) {
			return Err(Invalid::new(format!("{locale:?} is not a locale like fr, en or fr-FR")));
		}
		if out.iter().any(|l| l == locale) {
			return Err(Invalid::new(format!("{locale} is listed twice")));
		}
		out.push(locale.to_owned());
	}
	if out.is_empty() || out.len() > MAX_LOCALES {
		return Err(Invalid::new(format!("a brand speaks 1 to {MAX_LOCALES} locales")));
	}
	Ok(out)
}

pub fn default_locales() -> Vec<String> {
	DEFAULT_LOCALES.iter().map(|l| (*l).to_owned()).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn arithmetic_matches_kitstarts() {
		assert_eq!(mul_bp(101, 15_000), 152, "151.5 half up");
		assert_eq!(mul_bp(105, 9_000), 95, "94.5 half up");
		assert_eq!(mul_bp(105, 6_667), 70);
		assert_eq!(round_to(18_750, 100), 18_800);
		assert_eq!(round_to(8_415, 100), 8_400);
		assert_eq!(round_to(7, 5), 5);
		assert_eq!(round_to(8, 5), 10);
		assert_eq!(round_to(1234, 1), 1234);
	}

	#[test]
	fn dates_slugs_locales() {
		for ok in ["2026-10-01", "2024-02-29", "2000-02-29", "0000-01-01"] {
			assert!(is_date(ok), "{ok}");
		}
		for bad in ["2026-02-29", "1900-02-29", "2026-13-01", "2026-00-10", "2026-10-1", "2026-10-01T00:00:00Z", "２０２６-10-01"] {
			assert!(!is_date(bad), "{bad}");
		}
		assert!(is_slug("a-b_1") && !is_slug("A") && !is_slug("") && !is_slug(&"a".repeat(41)));
		assert!(is_locale("fr") && is_locale("fr-FR") && !is_locale("FR") && !is_locale("fr-fr") && !is_locale("fra"));
		assert_eq!(parse_locales("fr,en".split(',')).unwrap(), ["fr", "en"]);
		assert!(parse_locales(["fr", "fr"]).is_err());
		assert!(parse_locales([]).is_err());
		assert!(parse_locales(["french"]).is_err());
	}

	#[test]
	fn integers_as_javascript_reads_them() {
		assert_eq!(safe_integer(&json!(100.0)), Some(100));
		assert_eq!(safe_integer(&serde_json::from_str::<Value>("1e2").unwrap()), Some(100));
		assert_eq!(safe_integer(&json!(1.5)), None);
		assert_eq!(safe_integer(&json!(u64::MAX)), None);
		assert_eq!(safe_integer(&json!("100")), None);
	}

	#[test]
	fn labels_are_counted_in_utf16_and_must_not_be_blank() {
		let model = |label: &str| {
			json!({
				"format": 1, "currency": "EUR", "validFrom": "2026-10-01", "roundToCents": 1, "minimumCents": 0,
				"inputs": [{"id": "z", "kind": "add", "labels": {"fr": label}, "options": [{"id": "a", "labels": {"fr": "A"}, "addCents": 1}]}],
				"needs": {},
			})
		};
		assert!(PricingModel::parse(&model(&"é".repeat(120))).is_ok());
		// 60 astral characters are 120 UTF-16 units, 61 are 122.
		assert!(PricingModel::parse(&model(&"😀".repeat(60))).is_ok());
		let e = PricingModel::parse(&model(&"😀".repeat(61))).unwrap_err();
		assert_eq!(e[0].path, "model.inputs[0].labels.fr");
		assert_eq!(PricingModel::parse(&model("  ")).unwrap_err()[0].why, "a label of 1 to 120 characters");
	}

	#[test]
	fn problems_name_the_field() {
		let bad = json!({
			"format": 1, "currency": "EUR", "validFrom": "2026-10-01", "roundToCents": 1, "minimumCents": 0,
			"inputs": [{"id": "z", "kind": "add", "labels": {"fr": "Z"}, "options": [{"id": "a", "labels": {"fr": "A"}, "addCents": 1}]}],
			"needs": {"standard": {"kind": "estimate", "baseCents": 1, "inputs": ["z", "q", "z"]}},
		});
		let e = PricingModel::parse(&bad).unwrap_err();
		assert_eq!(
			e.iter().map(ToString::to_string).collect::<Vec<_>>(),
			["model.needs.standard.inputs: \"z\" is listed twice", "model.needs.standard.inputs[1]: no input \"q\""]
		);
		assert_eq!(PricingModel::parse(&json!([])).unwrap_err()[0].to_string(), "model: an object");
	}

	#[test]
	fn a_site_wants_every_label_in_its_locales() {
		let model = json!({
			"format": 1, "currency": "EUR", "validFrom": "2026-10-01", "roundToCents": 1, "minimumCents": 0,
			"inputs": [{"id": "z", "kind": "add", "labels": {"fr": "Z", "en": "Z"}, "options": [{"id": "a", "labels": {"fr": "A"}, "addCents": 1}]}],
			"needs": {},
		});
		assert!(PricingModel::parse_for(&model, &["fr".to_owned()]).is_ok());
		let e = PricingModel::parse_for(&model, &default_locales()).unwrap_err();
		assert_eq!(e[0].to_string(), "model.inputs[0].options[0].labels: no \"en\" label");
	}
}
