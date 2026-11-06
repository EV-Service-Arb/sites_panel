//! Booking without the I/O (FORM-VARIANTS-SPEC, "Booking providers contract", 2026-10-04):
//! the providers and how a place configures them, the statuses a lead's booking goes
//! through, and how its booking is folded from the facts about it.
//!
//! Provider-agnostic on purpose: a provider is a word of a closed vocabulary with its own URL
//! rule, and its adapter (a webhook it pushes, or a calendar the panel pulls) only ever hands
//! the engine a normalized [`BookingEvent`]. A place's config and `booking.requested` are held
//! to `contracts/vectors/booking`.

use std::collections::BTreeMap;

use jiff::{Timestamp, civil::Date};
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::{Invalid, ids::EventId};

vocabulary! {
	/// Where a slot is booked.
	#[ts(rename = "BookingProvider")]
	pub enum Provider, "provider is not one of manual, link, google_calendar, cal_com" {
		/// No external page: the site promises a call, and an operator records the slot.
		Manual => "manual",
		/// An external booking page on any https host; no adapter reads it back.
		Link => "link",
		/// A Google Calendar appointment schedule; its bookings are pulled from the calendar.
		GoogleCalendar => "google_calendar",
		/// Cal.com, ours (`cal.evinvest.ltd`) or the hosted cloud.
		CalCom => "cal_com",
	}
}

vocabulary! {
	/// A provider with a page of its own, which a place configures: every one but `manual`,
	/// which is always available.
	pub enum PageProvider, "not one of link, google_calendar, cal_com" {
		Link => "link",
		GoogleCalendar => "google_calendar",
		CalCom => "cal_com",
	}
}

impl Provider {
	/// Whether an adapter reports this provider's bookings (`booking.created`/`canceled`).
	pub fn has_adapter(self) -> bool {
		match self {
			Self::GoogleCalendar | Self::CalCom => true,
			Self::Manual | Self::Link => false,
		}
	}

	pub fn page(self) -> Option<PageProvider> {
		match self {
			Self::Manual => None,
			Self::Link => Some(PageProvider::Link),
			Self::GoogleCalendar => Some(PageProvider::GoogleCalendar),
			Self::CalCom => Some(PageProvider::CalCom),
		}
	}
}

impl From<PageProvider> for Provider {
	fn from(p: PageProvider) -> Self {
		match p {
			PageProvider::Link => Self::Link,
			PageProvider::GoogleCalendar => Self::GoogleCalendar,
			PageProvider::CalCom => Self::CalCom,
		}
	}
}

// ── a place's configuration ─────────────────────────────────────────────────────────────

/// The booking limits: the longest URL, the Cal.com hosts a page may be on, and the shape of
/// the lead ref a site names its lead by.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
	pub max_url: usize,
	/// Exact hostnames, lowercase; a subdomain is another host.
	pub cal_com_hosts: [&'static str; 2],
	/// A Cal.com user or event slug: one path segment, no dot segment.
	pub cal_com_segment: &'static str,
	/// `lead-<row>-<8 lowercase hex>`, the row 1 or more: [`is_lead_ref`].
	pub lead_ref: &'static str,
}

pub const LIMITS: Limits = Limits {
	max_url: 2048,
	cal_com_hosts: ["cal.evinvest.ltd", "cal.com"],
	cal_com_segment: "^[A-Za-z0-9][A-Za-z0-9._-]{0,99}$",
	lead_ref: "^lead-[1-9][0-9]{0,18}-[0-9a-f]{8}$",
};

vocabulary! {
	/// Why a place's `booking`, or a part of it, is refused.
	pub enum Code, "not a booking problem code" {
		/// Not `{default, providers}`, or a part that is not the object it should be.
		Shape => "shape",
		UnknownKey => "unknown_key",
		Required => "required",
		ProviderUnknown => "provider_unknown",
		ProviderShape => "provider_shape",
		ProviderExtra => "provider_extra",
		/// `manual` is always available, so never a key of `providers`.
		Manual => "manual",
		UrlType => "url_type",
		DefaultUnknown => "default_unknown",
		/// The default is a provider with a page the place does not configure.
		DefaultUnset => "default_unset",
		/// Too long, empty, or not printable ASCII.
		Format => "format",
		Scheme => "scheme",
		Fragment => "fragment",
		Backslash => "backslash",
		Userinfo => "userinfo",
		Ip => "ip",
		Port => "port",
		Host => "host",
		Google => "google",
		CalHost => "cal_host",
		CalPath => "cal_path",
	}
}

impl Code {
	fn why(self) -> String {
		match self {
			Self::Shape => "must be {\"default\": …, \"providers\": {…}} of objects".into(),
			Self::UnknownKey => "is not one of default, providers".into(),
			Self::Required => "is required".into(),
			Self::ProviderUnknown => "is not one of link, google_calendar, cal_com".into(),
			Self::ProviderShape => "must be an object".into(),
			Self::ProviderExtra => "is not a setting; only url".into(),
			Self::Manual => "manual is always available and is never a key of providers".into(),
			Self::UrlType => "must be a string".into(),
			Self::DefaultUnknown => "is not one of manual, link, google_calendar, cal_com".into(),
			Self::DefaultUnset => "must be manual or one of the providers set".into(),
			Self::Format => format!("must be an https:// URL of at most {} printable ASCII characters, without spaces", LIMITS.max_url),
			Self::Scheme => "must start with https://".into(),
			Self::Fragment => "must have no fragment (#…)".into(),
			Self::Backslash => "must have no backslash".into(),
			Self::Userinfo => "must have no user or password".into(),
			Self::Ip => "host must be a name, not an IP address".into(),
			Self::Port => "must have no port".into(),
			Self::Host => "host must be a domain name".into(),
			Self::Google => "must be a Google appointment schedule: https://calendar.app.google/… or https://calendar.google.com/calendar/appointments/…".into(),
			Self::CalHost => format!("must be a Cal.com page on {}", LIMITS.cal_com_hosts.join(" or ")),
			Self::CalPath => "must be a Cal.com page /<user>/<event>".into(),
		}
	}
}

/// One thing wrong with a place's `booking`: where below it (`""` for the setting itself,
/// `default`, `providers.cal_com.url`), and the code both checkers agree on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Problem {
	pub path: String,
	pub code: Code,
}

impl Problem {
	pub fn why(&self) -> String {
		self.code.why()
	}
}

/// `[A-Za-z0-9][A-Za-z0-9._-]{0,99}`: a Cal.com user or event slug.
fn is_cal_segment(s: &str) -> bool {
	(1..=100).contains(&s.len()) && s.starts_with(|c: char| c.is_ascii_alphanumeric()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Whether `url` may be `provider`'s booking page. Every provider with a page: at most
/// `max_url` printable ASCII; the literal `https://`; no `#` and no `\`; an authority without
/// `@`, `:` or `[`; a dotted DNS name of two labels at least, whose last is not a number
/// (`\d+`, `0x…`: an IPv4 address to a WHATWG parser); a query allowed. Hosts compare
/// lowercase. Then by provider: `google_calendar` — `calendar.app.google/…` (a short link) or
/// `calendar.google.com/calendar/appointments/…`; `cal_com` — one of `cal_com_hosts` exactly,
/// the path exactly `/<user>/<event>`; `link` — any host.
pub fn check_url(provider: PageProvider, raw: &str) -> Result<(), Code> {
	if raw.is_empty() || raw.len() > LIMITS.max_url || !raw.bytes().all(|b| b.is_ascii_graphic()) {
		return Err(Code::Format);
	}
	let rest = raw.strip_prefix("https://").ok_or(Code::Scheme)?;
	if raw.contains('#') {
		return Err(Code::Fragment);
	}
	if raw.contains('\\') {
		return Err(Code::Backslash);
	}
	let split = rest.find(['/', '?']).unwrap_or(rest.len());
	let (host, tail) = rest.split_at(split);
	if host.contains('@') {
		return Err(Code::Userinfo);
	}
	if host.starts_with('[') {
		return Err(Code::Ip);
	}
	if host.contains(':') {
		return Err(Code::Port);
	}
	let labels: Vec<&str> = host.split('.').collect();
	let label_ok = |l: &&str| (1..=63).contains(&l.len()) && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
	if host.len() > 253 || labels.len() < 2 || !labels.iter().all(label_ok) {
		return Err(Code::Host);
	}
	let last = labels.last().expect("two labels at least").to_ascii_lowercase();
	if last.bytes().all(|b| b.is_ascii_digit()) || last.starts_with("0x") {
		return Err(Code::Ip);
	}
	let host = host.to_ascii_lowercase();
	let path = tail.split('?').next().expect("split yields a first part");
	match provider {
		PageProvider::Link => Ok(()),
		PageProvider::GoogleCalendar => {
			const APPOINTMENTS: &str = "/calendar/appointments/";
			let ok = (host == "calendar.app.google" && path.len() > 1) || (host == "calendar.google.com" && path.len() > APPOINTMENTS.len() && path.starts_with(APPOINTMENTS));
			if ok { Ok(()) } else { Err(Code::Google) }
		}
		PageProvider::CalCom => {
			if !LIMITS.cal_com_hosts.contains(&host.as_str()) {
				return Err(Code::CalHost);
			}
			let segments: Vec<&str> = path.strip_prefix('/').unwrap_or_default().split('/').collect();
			if path.starts_with('/') && segments.len() == 2 && segments.iter().all(|s| is_cal_segment(s)) {
				Ok(())
			} else {
				Err(Code::CalPath)
			}
		}
	}
}

/// A provider's booking page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct BookingPage {
	pub url: String,
}

/// A place's `booking` setting: the providers it has a page on, and the one it offers by
/// default — `manual` or one of them. `manual` is always there, so never configured.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct BookingConfig {
	pub default: Provider,
	pub providers: BTreeMap<PageProvider, BookingPage>,
}

/// A site's `leadRef`, the panel's lead id for a site's lead: `lead-<row>-<8 hex>`, the row
/// 1 or more, the hex lowercase.
pub fn is_lead_ref(s: &str) -> bool {
	let Some(rest) = s.strip_prefix("lead-") else { return false };
	let Some((row, hex)) = rest.split_once('-') else { return false };
	let row_ok = (1..=19).contains(&row.len()) && row.bytes().all(|b| b.is_ascii_digit()) && !row.starts_with('0');
	row_ok && hex.len() == 8 && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `booking.requested@1`'s properties, checked: a lead ref, a provider, and a wish — date,
/// part of the day — for `manual` only.
pub fn requested(lead_ref: &str, provider: &str, date: Option<Date>, part: Option<&str>) -> Result<(Provider, Option<Date>, Option<DayPart>), Invalid> {
	if !is_lead_ref(lead_ref) {
		return Err(Invalid::new("properties.lead_ref is not lead-<row>-<8 lowercase hex>"));
	}
	let provider = Provider::parse(provider).map_err(|e| Invalid::new(format!("properties.{e}")))?;
	let part = part.map(DayPart::parse).transpose()?;
	if provider != Provider::Manual && (date.is_some() || part.is_some()) {
		return Err(Invalid::new("properties.preferred_date and preferred_part are for provider manual only"));
	}
	Ok((provider, date, part))
}

/// How far from the day it arrives a visitor's preferred day may be: two days back (clocks and
/// an outbox's delay), a year and a day ahead.
pub fn preferred_date_in_window(date: Date, today: Date) -> bool {
	let days = (date - today).get_days();
	(-2..=366).contains(&days)
}

/// A place's `booking` setting checked whole, every problem named by its path below the
/// setting.
pub fn check_config(v: &Value) -> Result<BookingConfig, Vec<Problem>> {
	let mut bad = Vec::new();
	let mut fail = |path: &str, code: Code| bad.push(Problem { path: path.to_owned(), code });
	let Value::Object(m) = v else {
		fail("", Code::Shape);
		return Err(bad);
	};
	if m.keys().any(|k| !matches!(k.as_str(), "default" | "providers")) {
		fail("", Code::UnknownKey);
	}
	let default = match m.get("default") {
		None => {
			fail("default", Code::Required);
			None
		}
		Some(raw) => {
			let p = raw.as_str().and_then(|s| Provider::parse(s).ok());
			if p.is_none() {
				fail("default", Code::DefaultUnknown);
			}
			p
		}
	};
	let mut providers = BTreeMap::new();
	match m.get("providers") {
		None => fail("providers", Code::Required),
		Some(Value::Object(ps)) =>
			for (name, conf) in ps {
				let path = format!("providers.{name}");
				let Ok(provider) = Provider::parse(name) else {
					fail(&path, Code::ProviderUnknown);
					continue;
				};
				let Value::Object(conf) = conf else {
					fail(&path, Code::ProviderShape);
					continue;
				};
				if conf.keys().any(|k| k != "url") {
					fail(&path, Code::ProviderExtra);
					continue;
				}
				let Some(page) = provider.page() else {
					fail(&path, Code::Manual);
					continue;
				};
				match conf.get("url") {
					None => fail(&format!("{path}.url"), Code::Required),
					Some(Value::String(url)) => match check_url(page, url) {
						Ok(()) => {
							providers.insert(page, BookingPage { url: url.clone() });
						}
						Err(code) => fail(&format!("{path}.url"), code),
					},
					Some(_) => fail(&format!("{path}.url"), Code::UrlType),
				}
			},
		Some(_) => fail("providers", Code::Shape),
	}
	// A default whose own entry is refused is blamed there, not twice.
	if let Some(page) = default.and_then(Provider::page)
		&& !providers.contains_key(&page)
		&& !bad.iter().any(|p| p.path.starts_with(&format!("providers.{page}")))
	{
		bad.push(Problem {
			path: "default".into(),
			code: Code::DefaultUnset,
		});
	}
	match default {
		Some(default) if bad.is_empty() => Ok(BookingConfig { default, providers }),
		_ => Err(bad),
	}
}

// ── a lead's booking ────────────────────────────────────────────────────────────────────

vocabulary! {
	/// Where a lead's booking stands.
	#[derive(Default)]
	pub enum BookingStatus, "booking status is not one of none, requested, booked, canceled, done, no_show" {
		#[default]
		None => "none",
		/// The site asked for a slot (`booking.requested`); nobody has one yet.
		Requested => "requested",
		Booked => "booked",
		Canceled => "canceled",
		Done => "done",
		NoShow => "no_show",
	}
}

impl BookingStatus {
	/// Whether an operator may do `action` to a booking that stands here: set a slot unless
	/// it is done; clear one that is there to clear (not from none, not once done); close a
	/// booked one only.
	pub fn allows(self, action: OperatorAction) -> bool {
		match action {
			OperatorAction::Set => self != Self::Done,
			OperatorAction::Clear => matches!(self, Self::Requested | Self::Booked | Self::Canceled | Self::NoShow),
			OperatorAction::Close(_) => self == Self::Booked,
		}
	}
}

/// How a booked slot ended, as an operator says (`booking.status_changed`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Closed {
	Done,
	NoShow,
	Canceled,
}

impl Closed {
	pub fn as_str(self) -> &'static str {
		self.status().as_str()
	}

	pub fn status(self) -> BookingStatus {
		match self {
			Self::Done => BookingStatus::Done,
			Self::NoShow => BookingStatus::NoShow,
			Self::Canceled => BookingStatus::Canceled,
		}
	}

	pub fn parse(raw: &str) -> Result<Self, Invalid> {
		[Self::Done, Self::NoShow, Self::Canceled]
			.into_iter()
			.find(|c| c.as_str() == raw)
			.ok_or_else(|| Invalid::new("status is not one of done, no_show, canceled"))
	}
}

/// What an operator does to a lead's booking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorAction {
	Set,
	Clear,
	Close(Closed),
}

vocabulary! {
	/// When in the day the visitor would like their slot.
	pub enum DayPart, "properties.preferred_part is not one of morning, afternoon, evening" {
		Morning => "morning",
		Afternoon => "afternoon",
		Evening => "evening",
	}
}

vocabulary! {
	/// How a provider's booking was joined to its lead.
	pub enum BookingMatch, "match is not one of ref, contact, manual" {
		/// By the ref the booking page carried (the lead's id).
		Ref => "ref",
		/// By the attendee's phone or email, one lead of the brand within the window.
		Contact => "contact",
		/// By an operator: a slot typed in, or a provider's booking attached by hand.
		Manual => "manual",
	}
}

/// A lead's booking now.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BookingState {
	pub status: BookingStatus,
	pub provider: Option<Provider>,
	pub start_at: Option<Timestamp>,
	pub end_at: Option<Timestamp>,
	/// The provider's booking, when the slot is one.
	pub external_ref: Option<String>,
	pub matched: Option<BookingMatch>,
	/// The visitor's wish, from the latest `booking.requested`.
	pub preferred_date: Option<Date>,
	pub preferred_part: Option<DayPart>,
}

/// One fact about a lead's booking, as its fold reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BookingItem {
	Requested {
		provider: Provider,
		date: Option<Date>,
		part: Option<DayPart>,
	},
	/// An operator's slot.
	Set {
		start: Timestamp,
		end: Option<Timestamp>,
	},
	Closed(Closed),
	Cleared,
	/// A provider's booking joined to the lead: booked (or moved) with its slot, or canceled.
	External {
		provider: Provider,
		external_ref: String,
		slot: Option<(Timestamp, Option<Timestamp>)>,
		matched: BookingMatch,
	},
}

/// Folds a lead's booking facts, in `(occurred_at, id)` order whatever order they come in.
/// What an operator could not have done from where the booking stood (closing one not
/// booked, clearing none) is passed over, so a race between two operators lands where the
/// first left it.
pub fn fold(items: &mut [(Timestamp, EventId, BookingItem)]) -> BookingState {
	items.sort_by_key(|(at, id, _)| (*at, id.raw()));
	let mut s = BookingState::default();
	for (_, _, item) in items.iter() {
		match item {
			BookingItem::Requested { provider, date, part } => {
				s.preferred_date = *date;
				s.preferred_part = *part;
				if matches!(s.status, BookingStatus::None | BookingStatus::Requested | BookingStatus::Canceled) {
					s = BookingState {
						status: BookingStatus::Requested,
						provider: Some(*provider),
						preferred_date: s.preferred_date,
						preferred_part: s.preferred_part,
						..BookingState::default()
					};
				}
			}
			BookingItem::Set { start, end } =>
				if s.status.allows(OperatorAction::Set) {
					s.status = BookingStatus::Booked;
					s.provider = Some(Provider::Manual);
					s.start_at = Some(*start);
					s.end_at = *end;
					s.external_ref = None;
					s.matched = Some(BookingMatch::Manual);
				},
			BookingItem::Closed(c) =>
				if s.status.allows(OperatorAction::Close(*c)) {
					s.status = c.status();
				},
			BookingItem::Cleared =>
				if s.status.allows(OperatorAction::Clear) {
					s = BookingState {
						preferred_date: s.preferred_date,
						preferred_part: s.preferred_part,
						..BookingState::default()
					};
				},
			BookingItem::External {
				provider,
				external_ref,
				slot,
				matched,
			} => {
				let same = s.external_ref.as_deref() == Some(external_ref.as_str());
				match slot {
					// A provider's change to a booking an operator already closed does not
					// reopen it.
					Some(_) if same && matches!(s.status, BookingStatus::Done | BookingStatus::NoShow) => {}
					Some((start, end)) => {
						s.status = BookingStatus::Booked;
						s.provider = Some(*provider);
						s.start_at = Some(*start);
						s.end_at = *end;
						s.external_ref = Some(external_ref.clone());
						s.matched = Some(*matched);
					}
					None =>
						if same && s.status == BookingStatus::Booked {
							s.status = BookingStatus::Canceled;
						},
				}
			}
		}
	}
	s
}

// ── a provider's bookings ───────────────────────────────────────────────────────────────

/// 1–1024 of `[A-Za-z0-9._:-]`: a provider's id of a booking (Google's event ids are
/// base32hex, Cal.com's uids alphanumeric with dashes). It sits in the clear.
pub fn is_external_ref(s: &str) -> bool {
	(1..=1024).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

/// 1–256 printable ASCII: a provider's revision of a booking (an etag, an update time).
pub fn is_version(s: &str) -> bool {
	(1..=256).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_graphic())
}

/// What a provider says changed, normalized: every adapter, push or pull, answers this.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Change {
	/// Booked, or moved to this slot.
	Booked {
		start: Timestamp,
		end: Option<Timestamp>,
		/// When the customer booked, if the provider says.
		booked_at: Option<Timestamp>,
	},
	Canceled,
}

/// Who booked, as the provider tells it: PII, sealed in the journal, compared with the
/// leads' to match, never in the clear.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Contact {
	pub name: Option<String>,
	/// Lowercase.
	pub email: Option<String>,
	/// E.164 ([`crate::phone::normalize`]).
	pub phone: Option<String>,
}

impl Contact {
	pub fn is_empty(&self) -> bool {
		self.name.is_none() && self.email.is_none() && self.phone.is_none()
	}
}

/// One provider's word about one booking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookingEvent {
	pub provider: Provider,
	pub external_ref: String,
	/// The provider's revision: the same revision seen twice is one journal event.
	pub version: String,
	/// When the provider says it changed.
	pub at: Timestamp,
	pub change: Change,
	/// The lead's id, when the booking page carried it back (a push provider's metadata).
	pub lead_ref: Option<String>,
	pub contact: Contact,
}

/// A provider's booking, folded from its own events and the operators' attachments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalBooking {
	/// Its latest slot, kept once canceled: what it was for.
	pub start: Timestamp,
	pub end: Option<Timestamp>,
	pub canceled: bool,
	pub booked_at: Option<Timestamp>,
	/// The lead and how it was found: the latest attachment, else the match the first event
	/// that named a lead was journaled with.
	pub lead: Option<(String, BookingMatch)>,
	/// The latest `booking.created`: what the attendee's PII is read from.
	pub created_event: EventId,
	pub first_event: EventId,
	pub last_event: EventId,
	pub last_event_at: Timestamp,
}

/// One event of a provider's booking, as [`fold_external`] reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalItem {
	Created {
		start: Timestamp,
		end: Option<Timestamp>,
		booked_at: Option<Timestamp>,
		lead: Option<(String, BookingMatch)>,
	},
	Canceled,
	Attached {
		lead: String,
	},
}

/// A provider's booking from its events, in `(occurred_at, id)` order; `None` while no
/// `booking.created` names it (a cancellation or an attachment alone is not a booking).
pub fn fold_external(items: &mut [(Timestamp, EventId, ExternalItem)]) -> Option<ExternalBooking> {
	items.sort_by_key(|(at, id, _)| (*at, id.raw()));
	let (first_created, start, end) = items.iter().enumerate().find_map(|(i, (_, _, item))| match item {
		ExternalItem::Created { start, end, .. } => Some((i, *start, *end)),
		ExternalItem::Canceled | ExternalItem::Attached { .. } => None,
	})?;
	let (first_at, first_id, _) = &items[0];
	let mut b = ExternalBooking {
		start,
		end,
		canceled: false,
		booked_at: None,
		lead: None,
		created_event: items[first_created].1,
		first_event: *first_id,
		last_event: *first_id,
		last_event_at: *first_at,
	};
	let mut attached = None;
	for (at, id, item) in items.iter() {
		match item {
			ExternalItem::Created { start, end, booked_at, lead } => {
				(b.start, b.end, b.canceled) = (*start, *end, false);
				b.booked_at = booked_at.or(b.booked_at);
				b.created_event = *id;
				if b.lead.is_none() {
					b.lead.clone_from(lead);
				}
			}
			ExternalItem::Canceled => b.canceled = true,
			ExternalItem::Attached { lead } => attached = Some(lead.clone()),
		}
		b.last_event = *id;
		b.last_event_at = *at;
	}
	if let Some(lead) = attached {
		b.lead = Some((lead, BookingMatch::Manual));
	}
	Some(b)
}

#[cfg(test)]
mod tests {
	use jiff::SignedDuration;
	use uuid::Uuid;

	use super::*;

	fn at(m: i64) -> Timestamp {
		"2026-10-05T08:00:00Z".parse::<Timestamp>().unwrap() + SignedDuration::from_mins(m)
	}

	fn id(n: u128) -> EventId {
		EventId::from_raw(Uuid::from_u128(n))
	}

	fn ext(slot: Option<i64>) -> BookingItem {
		BookingItem::External {
			provider: Provider::GoogleCalendar,
			external_ref: "ev1".into(),
			slot: slot.map(|m| (at(m), Some(at(m + 60)))),
			matched: BookingMatch::Contact,
		}
	}

	#[test]
	fn a_leads_booking() {
		let requested = BookingItem::Requested {
			provider: Provider::GoogleCalendar,
			date: Some("2026-10-07".parse().unwrap()),
			part: Some(DayPart::Morning),
		};
		let mut items = vec![(at(0), id(1), requested), (at(5), id(2), ext(Some(3000)))];
		let s = fold(&mut items);
		assert_eq!(
			(s.status, s.provider, s.matched),
			(BookingStatus::Booked, Some(Provider::GoogleCalendar), Some(BookingMatch::Contact))
		);
		assert_eq!(s.preferred_part, Some(DayPart::Morning), "the wish stays beside the slot");
		items.push((at(6), id(3), ext(Some(4000))));
		assert_eq!(fold(&mut items).start_at, Some(at(4000)), "moved");
		items.push((at(7), id(4), ext(None)));
		assert_eq!(fold(&mut items).status, BookingStatus::Canceled);
		items.reverse();
		assert_eq!(fold(&mut items).status, BookingStatus::Canceled, "whatever the order read in");

		let mut manual = vec![
			(at(0), id(1), BookingItem::Set { start: at(100), end: None }),
			(at(1), id(2), BookingItem::Closed(Closed::Done)),
			(at(2), id(3), BookingItem::Cleared),
			(at(3), id(4), BookingItem::Set { start: at(200), end: None }),
		];
		let s = fold(&mut manual);
		assert_eq!((s.status, s.start_at), (BookingStatus::Done, Some(at(100))), "a done booking is neither cleared nor set again");
		let mut raced = vec![(at(0), id(1), BookingItem::Closed(Closed::NoShow)), (at(1), id(2), BookingItem::Cleared)];
		assert_eq!(fold(&mut raced), BookingState::default(), "nothing booked: nothing to close or clear");
	}

	#[test]
	fn transitions() {
		use BookingStatus::*;
		let close = OperatorAction::Close(Closed::Done);
		for (from, set, clear, closes) in [
			(None, true, false, false),
			(Requested, true, true, false),
			(Booked, true, true, true),
			(Canceled, true, true, false),
			(Done, false, false, false),
			(NoShow, true, true, false),
		] {
			assert_eq!(
				(from.allows(OperatorAction::Set), from.allows(OperatorAction::Clear), from.allows(close)),
				(set, clear, closes),
				"{from:?}"
			);
		}
	}

	#[test]
	fn a_providers_booking() {
		let created = |m: i64, lead: Option<&str>| ExternalItem::Created {
			start: at(m),
			end: None,
			booked_at: Some(at(0)),
			lead: lead.map(|l| (l.to_owned(), BookingMatch::Contact)),
		};
		assert!(fold_external(&mut [(at(0), id(1), ExternalItem::Canceled)]).is_none());
		let mut items = vec![(at(0), id(1), created(100, None)), (at(1), id(2), created(200, Some("L-9")))];
		let b = fold_external(&mut items).unwrap();
		assert_eq!((b.start, b.canceled, b.lead.clone()), (at(200), false, Some(("L-9".into(), BookingMatch::Contact))));
		items.push((at(2), id(3), ExternalItem::Attached { lead: "L-1".into() }));
		items.push((at(3), id(4), ExternalItem::Canceled));
		let b = fold_external(&mut items).unwrap();
		assert_eq!((b.start, b.canceled, b.lead), (at(200), true, Some(("L-1".into(), BookingMatch::Manual))));
		assert_eq!((b.created_event, b.first_event, b.last_event), (id(2), id(1), id(4)));
	}

	#[test]
	fn refs_and_versions() {
		assert!(is_external_ref("abc_123-x.y:z"));
		assert!(!is_external_ref("a b") && !is_external_ref("") && !is_external_ref(&"a".repeat(1025)));
		assert!(is_version("\"3412345678901234\"") && !is_version("a b"));
	}
}
