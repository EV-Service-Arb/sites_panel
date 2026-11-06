//! A place's live settings: the half of a landing's place that the panel may change without a
//! release ([`PlaceLive`]). A site fetches them and lays them over its baked config, field by
//! field: whatever is left out, the site's own config supplies.
//!
//! One rule per field, held to `contracts/vectors/place` and run by both sides: the panel
//! refuses a field that breaks it and says why, so whoever edits sees it; a site drops that
//! field and keeps its baked one, so a page never fails on it.

use std::collections::BTreeMap;

use jiff::Timestamp;
use serde::Serialize;
use serde_json::{Map, Value};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
	booking::{self, BookingConfig},
	fact::Messenger,
	safe_integer,
};

vocabulary! {
	/// A day of the week, as the hours name it.
	pub enum DayOfWeek, "not a day of the week" {
		Monday => "Monday",
		Tuesday => "Tuesday",
		Wednesday => "Wednesday",
		Thursday => "Thursday",
		Friday => "Friday",
		Saturday => "Saturday",
		Sunday => "Sunday",
	}
}

/// The short names `panel place set --hours` takes, in [`DayOfWeek::ALL`] order.
const SHORT_DAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

/// The limits of a place's settings.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
	/// A line of text: a name, a street, a landmark.
	pub max_text: usize,
	pub max_url: usize,
	/// Rows of opening hours: one per day twice over (a lunch break) is the most a week needs.
	pub max_hours: usize,
	pub max_area: usize,
	pub max_locales: usize,
	/// `HH:MM`, 00:00 to 23:59.
	pub time: &'static str,
	/// A Telegram bot's username without the `@`, as `t.me/<bot>` takes it.
	pub telegram_bot: &'static str,
	/// `+` and 7 to 15 digits, the first not 0: what `tel:` and `wa.me` links take.
	pub e164: &'static str,
	/// The hosts Google's own review links live on: `g.page/r/…`, `search.google.com/local/writereview`.
	pub review_hosts: [&'static str; 2],
}

pub const LIMITS: Limits = Limits {
	max_text: 200,
	max_url: 2048,
	max_hours: 14,
	max_area: 200,
	max_locales: 8,
	time: "^([01][0-9]|2[0-3]):[0-5][0-9]$",
	telegram_bot: "^[A-Za-z][A-Za-z0-9_]{1,28}[Bb][Oo][Tt]$",
	e164: "^\\+[1-9][0-9]{6,14}$",
	review_hosts: ["g.page", "search.google.com"],
};

/// A row of opening hours, in the place's local time (Europe/Paris for every place in v1).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct OpeningHours {
	/// One at least, each once, in week order.
	pub days: Vec<DayOfWeek>,
	/// `HH:MM`.
	pub opens: String,
	/// After `opens`: a night across midnight is two rows.
	pub closes: String,
}

/// A storefront's address; its region and country are the site's.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Address {
	pub street: String,
	/// A French postal code: five digits.
	pub postal_code: String,
	pub locality: String,
}

/// Degrees, on the globe.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct Geo {
	pub lat: f64,
	pub lng: f64,
}

/// Google's rating for the place, with when it was read: the API's terms cap how long a copy
/// may be shown.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
	/// 1 to 5.
	pub value: f64,
	#[ts(type = "number")]
	pub count: i64,
	/// An RFC 3339 instant, as sent.
	pub fetched_at: String,
}

/// A place's live settings: only the fields that are set. Empty: the site serves its baked
/// config.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct PlaceLive {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub phone: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub whatsapp: Option<String>,
	/// The place's Telegram bot: its username, without the `@`.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub telegram: Option<String>,
	/// The landing's messenger buttons switched off where `false`; a messenger left out is on.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub messengers: Option<BTreeMap<Messenger, bool>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub hours: Option<Vec<OpeningHours>>,
	/// Commune names.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub service_area: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub address: Option<Address>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub geo: Option<Geo>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub storefront_photo: Option<String>,
	/// A text per locale; a site keeps it only when every locale it speaks is there.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub landmark: Option<BTreeMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rating: Option<Rating>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub booking: Option<BookingConfig>,
	/// Where the place asks for a Google review.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub review_url: Option<String>,
}

/// Every field a place's settings may set, by its wire name.
pub const FIELDS: [&str; 13] = [
	"phone", "whatsapp", "telegram", "messengers", "hours", "serviceArea", "address", "geo", "storefrontPhoto", "landmark", "rating", "booking", "reviewUrl",
];

vocabulary! {
	/// Why a field, or a part of one, is refused.
	pub enum Code, "not a place problem code" {
		Object => "object",
		UnknownField => "unknown_field",
		Required => "required",
		/// Not a string.
		Type => "type",
		Blank => "blank",
		TooLong => "too_long",
		/// A control character: a line of text is one line.
		Control => "control",
		E164 => "e164",
		TelegramBot => "telegram_bot",
		MessengerUnknown => "messenger_unknown",
		Boolean => "boolean",
		/// A list or an object that is not what the field takes.
		Shape => "shape",
		Empty => "empty",
		TooMany => "too_many",
		Day => "day",
		Duplicate => "duplicate",
		Time => "time",
		ClosesBeforeOpens => "closes_before_opens",
		PostalCode => "postal_code",
		Coordinate => "coordinate",
		HttpsUrl => "https_url",
		ReviewUrl => "review_url",
		Locale => "locale",
		RatingValue => "rating_value",
		RatingCount => "rating_count",
		Instant => "instant",
	}
}

/// Why a part of the settings is refused: a place's own rule, or the booking's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
	Place(Code),
	Booking(booking::Code),
}

impl Reason {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Place(c) => c.as_str(),
			Self::Booking(c) => c.as_str(),
		}
	}
}

/// One thing wrong with a place's settings: its path (`phone`, `hours[0].opens`,
/// `booking.providers.cal_com.url`) and why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Problem {
	pub path: String,
	pub reason: Reason,
}

/// Why each field was refused, by its path: what the editor shows beside the field.
pub type FieldErrors = BTreeMap<String, String>;

impl Problem {
	pub fn why(&self) -> String {
		let code = match self.reason {
			Reason::Booking(c) => return booking::Problem { path: String::new(), code: c }.why(),
			Reason::Place(c) => c,
		};
		match code {
			Code::Object => "must be an object".into(),
			Code::UnknownField => format!("is not a setting; one of {}", FIELDS.join(", ")),
			Code::Required => "is required".into(),
			Code::Type => "must be a string".into(),
			Code::Blank => "must not be blank".into(),
			Code::TooLong => "is too long".into(),
			Code::Control => "must be one line, without control characters".into(),
			Code::E164 => "must be E.164, e.g. +33612345678".into(),
			Code::TelegramBot => "must be a bot's username without the @: 5 to 32 of letters, digits and _, a letter first, ending in bot".into(),
			Code::MessengerUnknown => format!("is not a messenger; one of {}", Messenger::ALL.map(Messenger::as_str).join(", ")),
			Code::Boolean => "must be true or false".into(),
			Code::Shape => "is not in the shape this setting takes".into(),
			Code::Empty => "must not be empty; leave it out to keep the site's own".into(),
			Code::TooMany => "has too many entries".into(),
			Code::Day => format!("must be among {}", DayOfWeek::ALL.map(DayOfWeek::as_str).join(", ")),
			Code::Duplicate => "is named twice".into(),
			Code::Time => "must be HH:MM, 00:00 to 23:59".into(),
			Code::ClosesBeforeOpens => "must be after opens; split a night across midnight into two rows".into(),
			Code::PostalCode => "must be five digits".into(),
			Code::Coordinate => "must be a number on the globe: lat from -90 to 90, lng from -180 to 180".into(),
			Code::HttpsUrl => "must be an https:// URL".into(),
			Code::ReviewUrl => format!("must be an https:// review link on {}", LIMITS.review_hosts.join(" or ")),
			Code::Locale => "is not a locale like fr or en".into(),
			Code::RatingValue => "must be a number from 1 to 5".into(),
			Code::RatingCount => "must be a whole number, 0 or more".into(),
			Code::Instant => "must be an RFC 3339 instant, e.g. 2026-10-03T12:00:00Z".into(),
		}
	}
}

/// The problems found, collected by path.
#[derive(Default)]
struct Check {
	problems: Vec<Problem>,
}

impl Check {
	fn fail(&mut self, path: impl Into<String>, code: Code) {
		self.problems.push(Problem {
			path: path.into(),
			reason: Reason::Place(code),
		});
	}

	/// Run `f` on a fresh count of problems: whether it added none.
	fn clean<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> Option<T> {
		let before = self.problems.len();
		let out = f(self);
		(self.problems.len() == before).then_some(out)
	}

	/// A line of text, trimmed: not blank, at most `max` characters, no control characters.
	fn text(&mut self, path: &str, v: &Value, max: usize) -> String {
		let Value::String(s) = v else {
			self.fail(path, Code::Type);
			return String::new();
		};
		let s = s.trim();
		if s.is_empty() {
			self.fail(path, Code::Blank);
		} else if s.chars().count() > max {
			self.fail(path, Code::TooLong);
		} else if s.chars().any(char::is_control) {
			self.fail(path, Code::Control);
		}
		s.to_owned()
	}

	/// A text whose only fault is a `code` of its own: trimmed, and held to `ok`.
	fn word(&mut self, path: &str, v: &Value, max: usize, code: Code, ok: impl FnOnce(&str) -> bool) -> String {
		let s = v.as_str().map(str::trim).unwrap_or_default();
		if s.is_empty() || s.chars().count() > max || s.chars().any(char::is_control) || !ok(s) {
			self.fail(path, code);
		}
		s.to_owned()
	}

	/// An object with exactly `keys`; every other key and every missing one named.
	fn object<'a>(&mut self, path: &str, v: &'a Value, keys: &[&str]) -> Option<&'a Map<String, Value>> {
		let Value::Object(m) = v else {
			self.fail(path, Code::Shape);
			return None;
		};
		for k in m.keys().filter(|k| !keys.contains(&k.as_str())) {
			self.fail(format!("{path}.{k}"), Code::UnknownField);
		}
		for k in keys.iter().filter(|k| !m.contains_key(**k)) {
			self.fail(format!("{path}.{k}"), Code::Required);
		}
		(m.len() == keys.len() && keys.iter().all(|k| m.contains_key(*k))).then_some(m)
	}

	/// A list of `1..=max` entries.
	fn list<'a>(&mut self, path: &str, v: &'a Value, max: usize) -> &'a [Value] {
		match v {
			Value::Array(items) if items.is_empty() => self.fail(path, Code::Empty),
			Value::Array(items) if items.len() > max => self.fail(path, Code::TooMany),
			Value::Array(items) => return items,
			_ => self.fail(path, Code::Shape),
		}
		&[]
	}

	fn phone(&mut self, path: &str, v: &Value) -> String {
		self.word(path, v, 16, Code::E164, |s| {
			s.strip_prefix('+')
				.is_some_and(|d| (7..=15).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit()) && !d.starts_with('0'))
		})
	}

	fn telegram_bot(&mut self, path: &str, v: &Value) -> String {
		self.word(path, v, 32, Code::TelegramBot, |s| {
			let b = s.as_bytes();
			(5..=32).contains(&b.len()) && b[0].is_ascii_alphabetic() && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_') && b[b.len() - 3..].eq_ignore_ascii_case(b"bot")
		})
	}

	fn messengers(&mut self, path: &str, v: &Value) -> BTreeMap<Messenger, bool> {
		let Value::Object(m) = v else {
			self.fail(path, Code::Shape);
			return BTreeMap::new();
		};
		let mut out = BTreeMap::new();
		for (k, on) in m {
			match (Messenger::parse(k), on.as_bool()) {
				(Err(_), _) => self.fail(format!("{path}.{k}"), Code::MessengerUnknown),
				(Ok(_), None) => self.fail(format!("{path}.{k}"), Code::Boolean),
				(Ok(messenger), Some(on)) => {
					out.insert(messenger, on);
				}
			}
		}
		out
	}

	fn hours(&mut self, path: &str, v: &Value) -> Vec<OpeningHours> {
		let rows = self.list(path, v, LIMITS.max_hours);
		let mut out = Vec::with_capacity(rows.len());
		for (i, row) in rows.iter().enumerate() {
			let at = format!("{path}[{i}]");
			let Some(m) = self.object(&at, row, &["days", "opens", "closes"]) else { continue };
			let days = self.clean(|c| c.days(&format!("{at}.days"), &m["days"]));
			let opens = self.clean(|c| c.time(&format!("{at}.opens"), &m["opens"]));
			let closes = self.clean(|c| c.time(&format!("{at}.closes"), &m["closes"]));
			let (Some(days), Some(opens), Some(closes)) = (days, opens, closes) else { continue };
			// HH:MM sorts as the time it names.
			if closes <= opens {
				self.fail(format!("{at}.closes"), Code::ClosesBeforeOpens);
				continue;
			}
			out.push(OpeningHours { days, opens, closes });
		}
		out
	}

	/// A row's days: one at least, each once, put in week order.
	fn days(&mut self, path: &str, v: &Value) -> Vec<DayOfWeek> {
		let mut seen = Vec::new();
		for (i, d) in self.list(path, v, DayOfWeek::ALL.len()).iter().enumerate() {
			match d.as_str().map(DayOfWeek::parse) {
				Some(Ok(day)) if seen.contains(&day) => self.fail(format!("{path}[{i}]"), Code::Duplicate),
				Some(Ok(day)) => seen.push(day),
				_ => self.fail(format!("{path}[{i}]"), Code::Day),
			}
		}
		DayOfWeek::ALL.into_iter().filter(|d| seen.contains(d)).collect()
	}

	/// `HH:MM`, 00:00 to 23:59.
	fn time(&mut self, path: &str, v: &Value) -> String {
		let ok = |b: &[u8]| {
			b.len() == 5 && b[2] == b':' && b.iter().enumerate().all(|(i, c)| i == 2 || c.is_ascii_digit()) && matches!(b[0], b'0'..=b'2') && (b[0] != b'2' || b[1] <= b'3') && b[3] <= b'5'
		};
		match v.as_str() {
			Some(t) if ok(t.as_bytes()) => t.to_owned(),
			_ => {
				self.fail(path, Code::Time);
				String::new()
			}
		}
	}

	/// Commune names, at least one, each once (without regard to case).
	fn service_area(&mut self, path: &str, v: &Value) -> Vec<String> {
		let mut out: Vec<String> = Vec::new();
		for (i, n) in self.list(path, v, LIMITS.max_area).iter().enumerate() {
			let at = format!("{path}[{i}]");
			let Some(name) = self.clean(|c| c.text(&at, n, LIMITS.max_text)) else { continue };
			if out.iter().any(|o| o.to_lowercase() == name.to_lowercase()) {
				self.fail(at, Code::Duplicate);
			} else {
				out.push(name);
			}
		}
		out
	}

	fn address(&mut self, path: &str, v: &Value) -> Option<Address> {
		let m = self.object(path, v, &["street", "postalCode", "locality"])?;
		Some(Address {
			street: self.text(&format!("{path}.street"), &m["street"], LIMITS.max_text),
			postal_code: self.word(&format!("{path}.postalCode"), &m["postalCode"], 5, Code::PostalCode, |p| {
				p.len() == 5 && p.bytes().all(|b| b.is_ascii_digit())
			}),
			locality: self.text(&format!("{path}.locality"), &m["locality"], LIMITS.max_text),
		})
	}

	fn geo(&mut self, path: &str, v: &Value) -> Option<Geo> {
		let m = self.object(path, v, &["lat", "lng"])?;
		let mut coord = |k: &str, limit: f64| match m[k].as_f64() {
			Some(x) if x.abs() <= limit => x,
			_ => {
				self.fail(format!("{path}.{k}"), Code::Coordinate);
				0.0
			}
		};
		Some(Geo {
			lat: coord("lat", 90.0),
			lng: coord("lng", 180.0),
		})
	}

	fn landmark(&mut self, path: &str, v: &Value) -> BTreeMap<String, String> {
		let Value::Object(m) = v else {
			self.fail(path, Code::Shape);
			return BTreeMap::new();
		};
		if m.is_empty() {
			self.fail(path, Code::Empty);
		} else if m.len() > LIMITS.max_locales {
			self.fail(path, Code::TooMany);
		}
		let mut out = BTreeMap::new();
		for (locale, t) in m {
			let at = format!("{path}.{locale}");
			if !(locale.len() == 2 && locale.bytes().all(|b| b.is_ascii_lowercase())) {
				self.fail(at, Code::Locale);
				continue;
			}
			out.insert(locale.clone(), self.text(&at, t, LIMITS.max_text));
		}
		out
	}

	fn rating(&mut self, path: &str, v: &Value) -> Option<Rating> {
		let m = self.object(path, v, &["value", "count", "fetchedAt"])?;
		let value = m["value"].as_f64().filter(|x| (1.0..=5.0).contains(x));
		if value.is_none() {
			self.fail(format!("{path}.value"), Code::RatingValue);
		}
		let count = safe_integer(&m["count"]).filter(|n| *n >= 0);
		if count.is_none() {
			self.fail(format!("{path}.count"), Code::RatingCount);
		}
		let fetched_at = m["fetchedAt"].as_str().filter(|s| is_instant(s));
		if fetched_at.is_none() {
			self.fail(format!("{path}.fetchedAt"), Code::Instant);
		}
		Some(Rating {
			value: value?,
			count: count?,
			fetched_at: fetched_at?.to_owned(),
		})
	}

	fn booking(&mut self, path: &str, v: &Value) -> Option<BookingConfig> {
		booking::check_config(v)
			.map_err(|bad| {
				self.problems.extend(bad.into_iter().map(|p| Problem {
					path: if p.path.is_empty() { path.to_owned() } else { format!("{path}.{}", p.path) },
					reason: Reason::Booking(p.code),
				}))
			})
			.ok()
	}
}

/// The host of an absolute `https://` URL without userinfo or whitespace, lowercase.
fn https_host(s: &str) -> Option<String> {
	let authority = s.strip_prefix("https://")?.split(['/', '?', '#']).next().expect("split yields a first part");
	let ok = !authority.is_empty() && !authority.starts_with([':', '.']) && !authority.contains('@') && !s.contains(char::is_whitespace);
	ok.then(|| authority.to_ascii_lowercase())
}

/// `YYYY-MM-DDTHH:MM:SS`, an optional fraction of 1 to 9 digits, and `Z` or `±HH:MM`: an RFC
/// 3339 instant, on a day the calendar has, at a time of day the clock has (no leap second),
/// at an offset of less than a day.
pub fn is_instant(s: &str) -> bool {
	let b = s.as_bytes();
	let num = |r: std::ops::Range<usize>| -> Option<u32> {
		let d = b.get(r)?;
		d.iter().all(u8::is_ascii_digit).then(|| d.iter().fold(0, |n, c| n * 10 + u32::from(c - b'0')))
	};
	let clock = |at: usize, max_hour: u32| b.get(at + 2) == Some(&b':') && num(at..at + 2).is_some_and(|h| h <= max_hour) && num(at + 3..at + 5).is_some_and(|m| m <= 59);
	if b.len() < 20 || !s.get(..10).is_some_and(crate::pricing::is_date) || b[10] != b'T' || !clock(11, 23) || b[16] != b':' || num(17..19).is_none_or(|sec| sec > 59) {
		return false;
	}
	let mut i = 19;
	if b[i] == b'.' {
		let n = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
		if !(1..=9).contains(&n) {
			return false;
		}
		i += 1 + n;
	}
	match &b[i..] {
		b"Z" => true,
		[sign, ..] if b.len() == i + 6 && matches!(sign, b'+' | b'-') => clock(i + 1, 23),
		_ => false,
	}
}

impl PlaceLive {
	/// Checks a whole set of settings, field by field: the fields that pass, and every problem
	/// of those that do not. A field set to `null` is left out, as if absent.
	pub fn check(body: &Value) -> (Self, Vec<Problem>) {
		let mut c = Check::default();
		let mut live = Self::default();
		let Value::Object(fields) = body else {
			c.fail("", Code::Object);
			return (live, c.problems);
		};
		for (name, v) in fields.iter().filter(|(_, v)| !v.is_null()) {
			let name = name.as_str();
			match name {
				"phone" => live.phone = c.clean(|c| c.phone(name, v)),
				"whatsapp" => live.whatsapp = c.clean(|c| c.phone(name, v)),
				"telegram" => live.telegram = c.clean(|c| c.telegram_bot(name, v)),
				"messengers" => live.messengers = c.clean(|c| c.messengers(name, v)),
				"hours" => live.hours = c.clean(|c| c.hours(name, v)),
				"serviceArea" => live.service_area = c.clean(|c| c.service_area(name, v)),
				"address" => live.address = c.clean(|c| c.address(name, v)).flatten(),
				"geo" => live.geo = c.clean(|c| c.geo(name, v)).flatten(),
				"storefrontPhoto" => live.storefront_photo = c.clean(|c| c.word(name, v, LIMITS.max_url, Code::HttpsUrl, |s| https_host(s).is_some())),
				"landmark" => live.landmark = c.clean(|c| c.landmark(name, v)),
				"rating" => live.rating = c.clean(|c| c.rating(name, v)).flatten(),
				"booking" => live.booking = c.clean(|c| c.booking(name, v)).flatten(),
				"reviewUrl" =>
					live.review_url = c.clean(|c| {
						c.word(name, v, LIMITS.max_url, Code::ReviewUrl, |s| {
							https_host(s).is_some_and(|h| LIMITS.review_hosts.contains(&h.as_str()))
						})
					}),
				_ => c.fail(name, Code::UnknownField),
			}
		}
		(live, c.problems)
	}
}

/// A place's live settings as stored: an object of the fields set, in [`PlaceLive`]'s shape
/// when they were written. Empty: the site serves its baked config.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceSettings(Map<String, Value>);

impl PlaceSettings {
	/// Checks a whole set of settings (a full replace, not a patch); every bad field is named,
	/// not only the first.
	pub fn parse(body: &Value) -> Result<Self, FieldErrors> {
		match PlaceLive::check(body) {
			(live, problems) if problems.is_empty() => match serde_json::to_value(live).expect("settings are plain data") {
				Value::Object(m) => Ok(Self(m)),
				_ => unreachable!("a struct serializes to an object"),
			},
			(_, problems) => Err(problems.iter().map(|p| (p.path.clone(), p.why())).collect()),
		}
	}

	/// Settings as they were stored: an object, not checked again. The rules may have grown
	/// stricter since, and the history must still read.
	pub fn stored(value: Value) -> Option<Self> {
		match value {
			Value::Object(m) => Some(Self(m)),
			_ => None,
		}
	}

	pub fn as_json(&self) -> Value {
		Value::Object(self.0.clone())
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn get(&self, field: &str) -> Option<&Value> {
		self.0.get(field)
	}

	/// These settings with `set` laid over them and `clear` removed, checked whole: what the
	/// CLI does, one field at a time.
	pub fn patched(&self, set: Map<String, Value>, clear: &[String]) -> Result<Self, FieldErrors> {
		let mut next = self.0.clone();
		let mut errors = FieldErrors::new();
		for field in clear {
			if !FIELDS.contains(&field.as_str()) {
				errors.insert(field.clone(), format!("is not a setting; one of {}", FIELDS.join(", ")));
			}
			next.remove(field);
		}
		next.extend(set);
		let parsed = Self::parse(&Value::Object(next));
		match parsed {
			Ok(s) if errors.is_empty() => Ok(s),
			Ok(_) => Err(errors),
			Err(e) => {
				errors.extend(e);
				Err(errors)
			}
		}
	}
}

/// `whatsapp=on,telegram=off` → the `messengers` object (`panel place set --messengers`): each
/// messenger once, `on` or `off`. Checked again by [`PlaceSettings::parse`].
pub fn messengers_from_spec(spec: &str) -> Result<Value, String> {
	let mut out = Map::new();
	for part in spec.split(',').map(str::trim) {
		let bad = || format!("{part:?} is not like whatsapp=on or telegram=off");
		let (name, state) = part.split_once('=').ok_or_else(bad)?;
		let (name, state) = (name.trim(), state.trim());
		if Messenger::parse(name).is_err() {
			return Err(format!("{name:?} is not a messenger; one of {}", Messenger::ALL.map(Messenger::as_str).join(", ")));
		}
		let on = match state {
			"on" => true,
			"off" => false,
			_ => return Err(bad()),
		};
		if out.insert(name.to_owned(), Value::Bool(on)).is_some() {
			return Err(format!("{name} is named twice"));
		}
	}
	Ok(Value::Object(out))
}

/// `Mo-Fr 08:00-19:00,Sa 09:00-12:00` → the `hours` list: rows split by `,`, each a day or a
/// range of days (`Mo`…`Su`, week order) and a span. Checked again by [`PlaceSettings::parse`].
pub fn hours_from_spec(spec: &str) -> Result<Value, String> {
	let bad = |row: &str, why: &str| format!("{row:?}: {why}; rows look like 'Mo-Fr 08:00-19:00,Sa 09:00-12:00'");
	let mut rows = Vec::new();
	for row in spec.split(',').map(str::trim) {
		let (days, span) = row.split_once(char::is_whitespace).ok_or_else(|| bad(row, "a day or days, a space, a span"))?;
		let day = |d: &str| SHORT_DAYS.iter().position(|s| *s == d).ok_or_else(|| bad(row, "days are Mo Tu We Th Fr Sa Su"));
		let (first, last) = match days.split_once('-') {
			Some((a, b)) => (day(a)?, day(b)?),
			None => (day(days)?, day(days)?),
		};
		if last < first {
			return Err(bad(row, "a range of days runs in week order"));
		}
		let (opens, closes) = span.trim().split_once('-').ok_or_else(|| bad(row, "a span is HH:MM-HH:MM"))?;
		rows.push(serde_json::json!({ "days": DayOfWeek::ALL[first..=last].iter().map(|d| d.as_str()).collect::<Vec<_>>(), "opens": opens, "closes": closes }));
	}
	Ok(Value::Array(rows))
}

/// What a change did to a place.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeKind {
	/// Registered by hand (it may not have had a lead yet).
	Register,
	/// Its settings replaced.
	Set,
	/// The settings an earlier change found, put back.
	Revert,
	/// Taken off the sites: they answer it as gone.
	Withdraw,
	/// Back on the sites.
	Restore,
}

impl ChangeKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Register => "register",
			Self::Set => "set",
			Self::Revert => "revert",
			Self::Withdraw => "withdraw",
			Self::Restore => "restore",
		}
	}

	pub fn parse(raw: &str) -> Option<Self> {
		[Self::Register, Self::Set, Self::Revert, Self::Withdraw, Self::Restore].into_iter().find(|k| k.as_str() == raw)
	}
}

/// Who changed a place: a signed-in user (concierge id, and the email the history shows), or
/// the CLI on the pod.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Editor {
	User { id: Uuid, label: String },
	Cli,
}

impl Editor {
	/// What the history shows as `by`.
	pub fn label(&self) -> &str {
		match self {
			Self::User { label, .. } => label,
			Self::Cli => "cli",
		}
	}

	pub fn user_id(&self) -> Option<Uuid> {
		match self {
			Self::User { id, .. } => Some(*id),
			Self::Cli => None,
		}
	}
}

/// One entry of a place's history: what its settings were before and after, who, when.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
	pub id: Uuid,
	pub at: Timestamp,
	pub kind: ChangeKind,
	/// [`Editor::label`].
	pub by: String,
	pub before: PlaceSettings,
	pub after: PlaceSettings,
	/// The change a [`ChangeKind::Revert`] went back before.
	pub reverts: Option<Uuid>,
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	#[test]
	fn the_clis_patch_and_hours() {
		let current = PlaceSettings::parse(&json!({"phone": "+33423500640", "serviceArea": ["Royat"]})).unwrap();
		let hours = hours_from_spec("Mo-Fr 08:00-19:00, Sa 09:00-12:00").unwrap();
		assert_eq!(
			hours,
			json!([
				{"days": ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"], "opens": "08:00", "closes": "19:00"},
				{"days": ["Saturday"], "opens": "09:00", "closes": "12:00"},
			])
		);
		let set = Map::from_iter([("hours".to_owned(), hours)]);
		let next = current.patched(set, &["serviceArea".to_owned()]).unwrap();
		assert_eq!(
			next.as_json(),
			json!({"phone": "+33423500640", "hours": hours_from_spec("Mo-Fr 08:00-19:00,Sa 09:00-12:00").unwrap()})
		);
		assert!(current.patched(Map::new(), &["fax".to_owned()]).unwrap_err().contains_key("fax"));
		for bad in ["Mo-Fr", "Fr-Mo 08:00-19:00", "Mon 08:00-19:00", "Mo 08:00"] {
			assert!(hours_from_spec(bad).is_err(), "{bad}");
		}
		assert!(current.patched(Map::from_iter([("hours".to_owned(), hours_from_spec("Mo 19:00-08:00").unwrap())]), &[]).is_err());
	}

	#[test]
	fn the_clis_messengers() {
		assert_eq!(messengers_from_spec("whatsapp=on, telegram=off").unwrap(), json!({"whatsapp": true, "telegram": false}));
		for bad in ["whatsapp", "whatsapp=yes", "sms=on", "whatsapp=on,whatsapp=off", ""] {
			assert!(messengers_from_spec(bad).is_err(), "{bad:?}");
		}
		let current = PlaceSettings::parse(&json!({"telegram": "aquafix_devis_bot", "messengers": {"telegram": false}})).unwrap();
		let next = current.patched(Map::new(), &["telegram".to_owned(), "messengers".to_owned()]).unwrap();
		assert!(next.is_empty(), "both are fields the CLI clears");
	}

	#[test]
	fn change_kinds_round_trip() {
		for k in [ChangeKind::Register, ChangeKind::Set, ChangeKind::Revert, ChangeKind::Withdraw, ChangeKind::Restore] {
			assert_eq!(ChangeKind::parse(k.as_str()), Some(k));
		}
		assert_eq!(ChangeKind::parse("delete"), None);
	}
}
