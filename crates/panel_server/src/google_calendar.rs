//! The `google_calendar` booking adapter: a brand owner's Google Calendar, pulled
//! (`events.list` with a sync token; Google appointment schedules send no webhooks), its
//! appointment bookings read as [`BookingEvent`]s for [`panel::Panel::ingest_bookings`]. And
//! the owner's consent, asked from the editor ([`Consents`]), that yields the refresh token the
//! panel keeps sealed (`panel::Panel::connect_google_calendar`).
//!
//! ```text
//! every minute, per brand with a token: the brand's lease, due every GOOGLE_CALENDAR_SYNC_MINUTES
//!   refresh token ─ POST oauth2.googleapis.com/token ─ access token (cached until it expires)
//!   GET www.googleapis.com/calendar/v3/calendars/{id}/events?singleEvents&showDeleted
//!       first: timeMin = now − 7 days; then: syncToken; pages followed; 410 → a full pull again
//!   items ─ booking_of ─ BookingEvent ─ match ─ journal; nextSyncToken kept once journaled
//! ```
//!
//! **Which events are bookings** is [`looks_like_booking`], kept conservative, and its
//! premises are unconfirmed against a real appointment-schedule event (none was at hand):
//! a timed, single event (no recurrence, `eventType` default) with an attendee who is
//! neither the calendar (`self`), the organizer nor a room, not declining, and a marker of
//! a booking in its description — Google's "Booked by" / "Réservé par" lines, a link to
//! the schedule, or a "Téléphone" answer (the schedule's form must ask for it: the
//! contract). An ordinary meeting with a guest and none of those is left out. A cancelled
//! event comes back from a sync with its id alone, so a cancellation is passed on for any
//! id and the engine keeps only those of bookings it has. The phone is the line after (or
//! beside) a phone label in the description, normalized as the sites do.
//!
//! Secrets: `GOOGLE_OAUTH_CLIENT_ID` / `GOOGLE_OAUTH_CLIENT_SECRET` (the panel's OAuth client,
//! a Web client whose redirect is `GOOGLE_OAUTH_REDIRECT_URI`); each brand's refresh token is
//! data, sealed. Scope `calendar.events.readonly`: read, never write.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use eyre::WrapErr;
use jiff::{SignedDuration, Timestamp};
use panel::{
	Panel,
	booking::{BookingEvent, Change, Contact, GoogleConsent, Provider, PullError, PullSource, Pulled},
};
use panel_core::{phone, place::Editor};
use sa_core::ids::BrandId;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;
use zeroize::Zeroizing;

/// 256 random bits, base64url: a state, a PKCE verifier.
fn random_token() -> eyre::Result<String> {
	let mut raw = Zeroizing::new([0u8; 32]);
	getrandom::fill(raw.as_mut_slice()).map_err(|e| eyre::eyre!("the OS random source failed: {e}"))?;
	Ok(URL_SAFE_NO_PAD.encode(raw.as_slice()))
}

/// The S256 code challenge of a PKCE verifier.
fn pkce_challenge(verifier: &str) -> String {
	URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const API_BASE: &str = "https://www.googleapis.com/calendar/v3";
pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events.readonly";

/// How far back a full pull looks.
pub const FULL_PULL_DAYS: i64 = 7;
/// Pages of 250 one pull follows at most: past it the pull fails rather than run on.
const MAX_PAGES: usize = 40;
/// How often a replica asks whether a brand's sync is due; the lease makes it once per period.
const TICK: Duration = Duration::from_secs(60);

/// One brand's calendar.
pub struct BrandCalendar {
	pub refresh_token: Zeroizing<String>,
	/// `primary`, or a calendar the owner shares.
	pub calendar_id: String,
}

impl std::fmt::Debug for BrandCalendar {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("BrandCalendar").field("calendar_id", &self.calendar_id).finish_non_exhaustive()
	}
}

/// The adapter: the panel's OAuth client and each brand's calendar.
#[derive(Clone)]
pub struct GoogleCalendar {
	http: reqwest::Client,
	token_url: String,
	api_base: String,
	client_id: String,
	client_secret: Arc<Zeroizing<String>>,
	brands: Arc<BTreeMap<BrandId, BrandCalendar>>,
	/// Access tokens, by brand, until they expire.
	tokens: Arc<Mutex<BTreeMap<BrandId, Cached>>>,
}

/// An access token and when it stops being used.
type Cached = (Zeroizing<String>, Timestamp);

impl std::fmt::Debug for GoogleCalendar {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("GoogleCalendar")
			.field("api_base", &self.api_base)
			.field("brands", &self.brands.keys().collect::<Vec<_>>())
			.finish_non_exhaustive()
	}
}

fn client() -> eyre::Result<reqwest::Client> {
	Ok(reqwest::Client::builder()
		.connect_timeout(Duration::from_secs(5))
		.timeout(Duration::from_secs(30))
		.redirect(reqwest::redirect::Policy::none())
		.build()?)
}

impl GoogleCalendar {
	/// `token_url` and `api_base`: Google's ([`TOKEN_URL`], [`API_BASE`]), or a test's fake.
	pub fn new(token_url: &str, api_base: &str, client_id: &str, client_secret: &str, brands: BTreeMap<BrandId, BrandCalendar>) -> eyre::Result<Self> {
		Ok(Self {
			http: client()?,
			token_url: token_url.to_owned(),
			api_base: api_base.trim_end_matches('/').to_owned(),
			client_id: client_id.to_owned(),
			client_secret: Arc::new(Zeroizing::new(client_secret.to_owned())),
			brands: Arc::new(brands),
			tokens: Arc::default(),
		})
	}

	/// This adapter over the brands `consents` name, its access tokens kept.
	pub fn of(&self, consents: Vec<GoogleConsent>) -> Self {
		let brands = consents
			.into_iter()
			.map(|c| {
				let cal = BrandCalendar {
					refresh_token: c.refresh_token,
					calendar_id: c.calendar_id,
				};
				(c.brand, cal)
			})
			.collect();
		Self {
			brands: Arc::new(brands),
			..self.clone()
		}
	}

	/// The brands with a calendar.
	pub fn brands(&self) -> Vec<BrandId> {
		self.brands.keys().cloned().collect()
	}

	async fn access_token(&self, brand: &BrandId, cal: &BrandCalendar) -> eyre::Result<Zeroizing<String>> {
		let now = Timestamp::now();
		if let Some((token, until)) = self.tokens.lock().await.get(brand)
			&& *until > now
		{
			return Ok(token.clone());
		}
		let form = url::form_urlencoded::Serializer::new(String::new())
			.append_pair("client_id", &self.client_id)
			.append_pair("client_secret", &self.client_secret)
			.append_pair("refresh_token", &cal.refresh_token)
			.append_pair("grant_type", "refresh_token")
			.finish();
		let answer = token_request(&self.http, &self.token_url, form)
			.await
			.wrap_err_with(|| format!("refreshing {brand}'s Google access token"))?;
		let token = answer
			.get("access_token")
			.and_then(Value::as_str)
			.ok_or_else(|| eyre::eyre!("Google's token answer for {brand} has no access_token"))?;
		let token = Zeroizing::new(token.to_owned());
		// A minute short of what Google says, so a token is never used at its last second.
		let secs = answer.get("expires_in").and_then(Value::as_i64).unwrap_or(3600).saturating_sub(60).max(0);
		self.tokens.lock().await.insert(brand.clone(), (token.clone(), now + SignedDuration::from_secs(secs)));
		Ok(token)
	}
}

/// POSTs a token request's form and reads its JSON; a refusal says Google's `error`, never
/// the form (it holds the secrets).
async fn token_request(http: &reqwest::Client, url: &str, form: String) -> eyre::Result<Value> {
	let res = http
		.post(url)
		.header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
		.body(form)
		.send()
		.await
		.map_err(|e| eyre::eyre!("the token endpoint did not answer: {e}"))?;
	let status = res.status();
	let body: Value = res.json().await.unwrap_or(Value::Null);
	if !status.is_success() {
		let why = body.get("error").and_then(Value::as_str).unwrap_or("no reason given");
		eyre::bail!("the token endpoint answered {status}: {why}");
	}
	Ok(body)
}

impl PullSource for GoogleCalendar {
	fn provider(&self) -> Provider {
		Provider::GoogleCalendar
	}

	async fn pull(&self, brand: &BrandId, cursor: Option<&str>, now: Timestamp) -> Result<Pulled, PullError> {
		let cal = self.brands.get(brand).ok_or_else(|| eyre::eyre!("{brand} has no Google calendar configured"))?;
		let token = self.access_token(brand, cal).await?;
		let calendar: String = url::form_urlencoded::byte_serialize(cal.calendar_id.as_bytes()).collect();
		let base = format!("{}/calendars/{calendar}/events", self.api_base);
		let mut pulled = Pulled::default();
		let mut page: Option<String> = None;
		for _ in 0..MAX_PAGES {
			let mut url = url::Url::parse(&base).wrap_err("the Calendar API's URL")?;
			{
				let mut q = url.query_pairs_mut();
				q.append_pair("singleEvents", "true").append_pair("showDeleted", "true").append_pair("maxResults", "250");
				match cursor {
					Some(c) => q.append_pair("syncToken", c),
					None => q.append_pair("timeMin", &(now - SignedDuration::from_hours(24 * FULL_PULL_DAYS)).to_string()),
				};
				if let Some(p) = &page {
					q.append_pair("pageToken", p);
				}
			}
			let res = self
				.http
				.get(url)
				.bearer_auth(token.as_str())
				.send()
				.await
				.map_err(|e| eyre::eyre!("the Calendar API did not answer: {e}"))?;
			let status = res.status();
			if status == reqwest::StatusCode::GONE {
				return Err(PullError::CursorExpired);
			}
			if status == reqwest::StatusCode::UNAUTHORIZED {
				self.tokens.lock().await.remove(brand);
			}
			if !status.is_success() {
				let why: String = res.text().await.unwrap_or_default().chars().take(300).collect();
				return Err(eyre::eyre!("the Calendar API answered {status} for {brand}: {why}").into());
			}
			let body: Value = res.json().await.wrap_err("the Calendar API's answer is not JSON")?;
			for item in body.get("items").and_then(Value::as_array).into_iter().flatten() {
				pulled.events.extend(booking_of(item, now));
			}
			if let Some(next) = body.get("nextPageToken").and_then(Value::as_str) {
				page = Some(next.to_owned());
				continue;
			}
			pulled.cursor = body.get("nextSyncToken").and_then(Value::as_str).map(str::to_owned);
			return Ok(pulled);
		}
		Err(eyre::eyre!("{brand}'s calendar has more than {MAX_PAGES} pages of changes; pull with --full").into())
	}
}

// ── what a calendar event says ──────────────────────────────────────────────────────────

/// Lines that mark an appointment booking in an event's description, lowercase: Google's own
/// lines ("Booked by", in French "Réservé par") and links to an appointment schedule.
const BOOKING_MARKERS: [&str; 5] = ["booked by", "réservé par", "réservée par", "calendar.app.google", "calendar.google.com/calendar/appointments"];

/// Labels of a phone answer, lowercase and unaccented.
const PHONE_LABELS: [&str; 7] = ["telephone", "phone", "phone number", "numero de telephone", "tel", "mobile", "portable"];

/// An event's description as text: `<br>` and block ends are line breaks, other tags gone,
/// the few entities Google writes read.
fn description_text(html: &str) -> String {
	let mut out = String::with_capacity(html.len());
	let mut rest = html;
	while let Some(open) = rest.find('<') {
		out.push_str(&rest[..open]);
		let Some(close) = rest[open..].find('>') else {
			out.push_str(&rest[open..]);
			rest = "";
			break;
		};
		let tag = rest[open + 1..open + close].trim_start_matches('/').to_ascii_lowercase();
		if tag.starts_with("br") || tag.starts_with('p') || tag.starts_with("div") || tag.starts_with("li") {
			out.push('\n');
		}
		rest = &rest[open + close + 1..];
	}
	out.push_str(rest);
	out.replace("&nbsp;", " ")
		.replace("&amp;", "&")
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&#39;", "'")
		.replace("&quot;", "\"")
}

fn unaccented(s: &str) -> String {
	s.chars()
		.map(|c| match c {
			'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' => 'e',
			'à' | 'â' => 'a',
			'ô' => 'o',
			'î' | 'ï' => 'i',
			'û' | 'ù' => 'u',
			c => c.to_ascii_lowercase(),
		})
		.collect()
}

fn is_phone_label(s: &str) -> bool {
	let label = unaccented(s.trim().trim_end_matches([':', '*', ' ']).trim());
	PHONE_LABELS.contains(&label.trim_end_matches('.'))
}

/// The phone the booking form's "Téléphone" answer gave: the line after a phone label, or the
/// rest of the label's own line (`Téléphone : 06 …`), normalized; `None` when there is no
/// label or its answer is not a number.
pub fn labeled_phone(description: &str) -> Option<String> {
	let text = description_text(description);
	let lines: Vec<&str> = text.lines().map(str::trim).collect();
	for (i, line) in lines.iter().enumerate() {
		if is_phone_label(line) {
			return lines[i + 1..].iter().find(|l| !l.is_empty()).and_then(|l| phone::normalize(l));
		}
		if let Some((label, rest)) = line.split_once(':')
			&& is_phone_label(label)
		{
			return phone::normalize(rest.trim());
		}
	}
	None
}

/// The guest who booked: an attendee with an email who is not the calendar itself, not the
/// organizer, not a room, and has not declined.
fn guest(item: &Value) -> Option<&Value> {
	item.get("attendees")?.as_array()?.iter().find(|a| {
		let yes = |k: &str| a.get(k).and_then(Value::as_bool).unwrap_or(false);
		a.get("email").and_then(Value::as_str).is_some_and(|e| e.contains('@'))
			&& !yes("self")
			&& !yes("organizer")
			&& !yes("resource")
			&& a.get("responseStatus").and_then(Value::as_str) != Some("declined")
	})
}

/// Whether a (not cancelled) calendar event is an appointment booking: see the module.
pub fn looks_like_booking(item: &Value) -> bool {
	let timed = item.pointer("/start/dateTime").is_some_and(Value::is_string);
	let single = item.get("recurringEventId").is_none() && item.get("recurrence").is_none();
	let plain = item.get("eventType").and_then(Value::as_str).is_none_or(|t| t == "default");
	if !(timed && single && plain) || guest(item).is_none() {
		return false;
	}
	let description = item.get("description").and_then(Value::as_str).unwrap_or_default();
	let lower = description_text(description).to_lowercase();
	BOOKING_MARKERS.iter().any(|m| lower.contains(m)) || labeled_phone(description).is_some()
}

fn instant(item: &Value, pointer: &str) -> Option<Timestamp> {
	item.pointer(pointer)?.as_str()?.parse().ok()
}

/// A calendar event as a booking event: a booking booked or moved, a cancellation (of
/// whatever id: the engine keeps only its own), or `None` for anything else.
pub fn booking_of(item: &Value, now: Timestamp) -> Option<BookingEvent> {
	let id = item.get("id")?.as_str()?.to_owned();
	let at = instant(item, "/updated").unwrap_or(now);
	// The etag changes with every change of the event; `updated` stands in where it is absent.
	let version = item
		.get("etag")
		.and_then(Value::as_str)
		.map(str::to_owned)
		.or_else(|| item.get("updated").and_then(Value::as_str).map(str::to_owned))?;
	let base = |change| BookingEvent {
		provider: Provider::GoogleCalendar,
		external_ref: id.clone(),
		version: version.clone(),
		at,
		change,
		lead_ref: None,
		contact: Contact::default(),
	};
	if item.get("status").and_then(Value::as_str) == Some("cancelled") {
		return Some(base(Change::Canceled));
	}
	if !looks_like_booking(item) {
		return None;
	}
	let start = instant(item, "/start/dateTime")?;
	let end = instant(item, "/end/dateTime").filter(|e| *e > start);
	let guest = guest(item)?;
	let mut ev = base(Change::Booked {
		start,
		end,
		booked_at: instant(item, "/created"),
	});
	ev.contact = Contact {
		name: guest.get("displayName").and_then(Value::as_str).map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()),
		email: guest.get("email").and_then(Value::as_str).map(|e| e.trim().to_lowercase()),
		phone: item.get("description").and_then(Value::as_str).and_then(labeled_phone),
	};
	Some(ev)
}

// ── the schedule in `serve` ─────────────────────────────────────────────────────────────

/// Pulls every consented brand's calendar until `shutdown`: each minute, the brands whose sync is
/// due (`every`) and not held by another process, read from the consents as they stand then.
pub async fn run(panel: Panel, google: GoogleCalendar, every: SignedDuration, shutdown: watch::Receiver<bool>) {
	let holder = Uuid::now_v7();
	crate::every(TICK, shutdown, "the google calendar sync", || async {
		let google = google.of(panel.google_calendars().await?);
		for brand in google.brands() {
			// One brand's failure is reported and does not hold the others back.
			if let Err(e) = panel.sync_bookings(&google, &brand, holder, Timestamp::now(), every, false, false).await {
				crate::report(&e, "a google calendar sync");
			}
		}
		Ok(())
	})
	.await;
}

// ── consent, from the editor ────────────────────────────────────────────────────────────

/// How long a consent asked may wait for Google's redirect.
const CONSENT_WITHIN: SignedDuration = SignedDuration::from_mins(10);

/// The owners' consents: Google's consent page, then its redirect to `redirect_uri` (the
/// conductor's path to [`crate::booking`]'s callback, registered on the OAuth client), traded
/// there for the refresh token. A consent asked waits in memory: one pod.
pub struct Consents {
	google: GoogleCalendar,
	redirect_uri: url::Url,
	asked: std::sync::Mutex<BTreeMap<String, Asked>>,
}

struct Asked {
	brand: BrandId,
	calendar_id: String,
	verifier: Zeroizing<String>,
	return_to: url::Url,
	by: Editor,
	until: Timestamp,
}

/// Why a consent's redirect was not taken.
#[derive(Debug, thiserror::Error)]
pub enum ConsentError {
	/// `return_to` is not on the redirect's origin: the editor's page is the conductor's.
	#[error("return_to is not a page of {0}")]
	Elsewhere(String),
	/// No consent asked under this state, or asked more than 10 minutes ago.
	#[error("no consent waits under this state: ask again")]
	Unknown,
	/// Google said no, or gave no refresh token.
	#[error("{0}")]
	Refused(String),
	#[error(transparent)]
	Internal(#[from] eyre::Report),
}

impl Consents {
	pub fn new(google: GoogleCalendar, redirect_uri: url::Url) -> Self {
		Self {
			google,
			redirect_uri,
			asked: std::sync::Mutex::default(),
		}
	}

	/// Google's consent URL for `brand`'s owner: offline access (a refresh token), the consent
	/// screen every time (so a refresh token is always given), PKCE.
	pub(crate) fn ask(&self, brand: BrandId, calendar_id: String, return_to: url::Url, by: Editor, now: Timestamp) -> Result<String, ConsentError> {
		if return_to.origin() != self.redirect_uri.origin() {
			return Err(ConsentError::Elsewhere(self.redirect_uri.origin().ascii_serialization()));
		}
		let state = random_token()?;
		let verifier = Zeroizing::new(random_token()?);
		let url = consent_url(&self.google.client_id, self.redirect_uri.as_str(), &state, &pkce_challenge(&verifier))?;
		let mut asked = self.asked.lock().expect("no panic while held");
		asked.retain(|_, a| a.until > now);
		asked.insert(
			state,
			Asked {
				brand,
				calendar_id,
				verifier,
				return_to,
				by,
				until: now + CONSENT_WITHIN,
			},
		);
		Ok(url)
	}

	/// Google's redirect: the consent asked under `state`, its `code` traded for the refresh token.
	/// Answers the consent, who asked it, and the page to send them back to.
	pub(crate) async fn answer(&self, state: &str, code: Result<&str, &str>, now: Timestamp) -> Result<(GoogleConsent, Editor, url::Url), ConsentError> {
		let asked = self
			.asked
			.lock()
			.expect("no panic while held")
			.remove(state)
			.filter(|a| a.until > now)
			.ok_or(ConsentError::Unknown)?;
		let code = code.map_err(|e| ConsentError::Refused(format!("Google answered {e}")))?;
		let form = url::form_urlencoded::Serializer::new(String::new())
			.append_pair("client_id", &self.google.client_id)
			.append_pair("client_secret", &self.google.client_secret)
			.append_pair("code", code)
			.append_pair("code_verifier", &asked.verifier)
			.append_pair("redirect_uri", self.redirect_uri.as_str())
			.append_pair("grant_type", "authorization_code")
			.finish();
		let answer = token_request(&self.google.http, &self.google.token_url, form)
			.await
			.map_err(|e| ConsentError::Refused(format!("trading the consent's code: {e}")))?;
		let refresh = answer
			.get("refresh_token")
			.and_then(Value::as_str)
			.ok_or_else(|| ConsentError::Refused("Google gave no refresh token: revoke the panel's access in the Google account and ask again".into()))?;
		let consent = GoogleConsent {
			brand: asked.brand,
			calendar_id: asked.calendar_id,
			refresh_token: Zeroizing::new(refresh.to_owned()),
		};
		Ok((consent, asked.by, asked.return_to))
	}
}

fn consent_url(client_id: &str, redirect_uri: &str, state: &str, challenge: &str) -> eyre::Result<String> {
	let mut url = url::Url::parse(AUTH_URL).wrap_err("Google's consent URL")?;
	url.query_pairs_mut()
		.append_pair("client_id", client_id)
		.append_pair("redirect_uri", redirect_uri)
		.append_pair("response_type", "code")
		.append_pair("scope", SCOPE)
		.append_pair("access_type", "offline")
		.append_pair("prompt", "consent")
		.append_pair("state", state)
		.append_pair("code_challenge", challenge)
		.append_pair("code_challenge_method", "S256");
	Ok(url.into())
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	fn now() -> Timestamp {
		"2026-10-05T12:00:00Z".parse().unwrap()
	}

	/// What an appointment-schedule booking is assumed to look like (see the module): the
	/// owner organizes, the customer is a guest, Google's "Réservé par" block and the form's
	/// answers in the description.
	fn appointment() -> Value {
		json!({
			"id": "a1b2c3d4e5",
			"etag": "\"3456789012345678\"",
			"status": "confirmed",
			"created": "2026-10-05T09:00:00.000Z",
			"updated": "2026-10-05T09:00:01.000Z",
			"summary": "Devis ménage (Jean Dupont)",
			"description": "Réservé par<br>Jean Dupont<br>jean.dupont@example.fr<br><br><b>Téléphone</b><br>06 12 34 56 78",
			"start": {"dateTime": "2026-10-08T10:00:00+02:00", "timeZone": "Europe/Paris"},
			"end": {"dateTime": "2026-10-08T10:30:00+02:00", "timeZone": "Europe/Paris"},
			"organizer": {"email": "owner@vifnet.fr", "self": true},
			"attendees": [
				{"email": "owner@vifnet.fr", "organizer": true, "self": true, "responseStatus": "accepted"},
				{"email": "Jean.Dupont@Example.fr", "displayName": "Jean Dupont", "responseStatus": "accepted"}
			]
		})
	}

	#[test]
	fn an_appointment_is_a_booking() {
		let ev = booking_of(&appointment(), now()).unwrap();
		assert_eq!(ev.external_ref, "a1b2c3d4e5");
		assert_eq!(ev.version, "\"3456789012345678\"");
		let Change::Booked { start, end, booked_at } = ev.change else { panic!("{ev:?}") };
		assert_eq!(start.to_string(), "2026-10-08T08:00:00Z");
		assert_eq!(end.unwrap().to_string(), "2026-10-08T08:30:00Z");
		assert_eq!(booked_at.unwrap().to_string(), "2026-10-05T09:00:00Z");
		assert_eq!(ev.contact.email.as_deref(), Some("jean.dupont@example.fr"));
		assert_eq!(ev.contact.phone.as_deref(), Some("+33612345678"));
		assert_eq!(ev.contact.name.as_deref(), Some("Jean Dupont"));
	}

	#[test]
	fn what_is_not_a_booking() {
		let with = |f: &dyn Fn(&mut Value)| {
			let mut e = appointment();
			f(&mut e);
			booking_of(&e, now())
		};
		assert!(
			with(&|e| e["description"] = json!("Weekly sync with the accountant")).is_none(),
			"a meeting with a guest and no marker"
		);
		assert!(
			with(&|e| e["attendees"] = json!([{"email": "owner@vifnet.fr", "self": true, "organizer": true}])).is_none(),
			"nobody booked it"
		);
		assert!(with(&|e| e["attendees"][1]["responseStatus"] = json!("declined")).is_none());
		assert!(with(&|e| e["attendees"][1]["resource"] = json!(true)).is_none(), "a room");
		assert!(with(&|e| e["recurringEventId"] = json!("r1")).is_none(), "a series");
		assert!(with(&|e| e["eventType"] = json!("outOfOffice")).is_none());
		assert!(with(&|e| e["start"] = json!({"date": "2026-10-08"})).is_none(), "all day");
		assert!(
			with(&|e| e["description"] = json!("Booked by\nJean\njean@example.fr")).is_some(),
			"Google's English line is a marker too"
		);
		assert!(with(&|e| e["description"] = json!("Téléphone : +33 6 12 34 56 78")).is_some(), "the form's phone answer alone");
	}

	#[test]
	fn a_cancellation_is_passed_on_with_its_id() {
		let ev = booking_of(&json!({"id": "a1b2c3d4e5", "etag": "\"9\"", "status": "cancelled"}), now()).unwrap();
		assert_eq!((ev.change, ev.at), (Change::Canceled, now()));
		assert!(booking_of(&json!({"status": "cancelled"}), now()).is_none(), "no id, nothing to cancel");
	}

	#[test]
	fn phone_labels() {
		for (description, want) in [
			("Téléphone\n06 12 34 56 78", Some("+33612345678")),
			("<p>Phone number:</p><p>+44 7911 123456</p>", Some("+447911123456")),
			("Numéro de téléphone * : 0033 6 12 34 56 78", Some("+33612345678")),
			("Tel.: 06.12.34.56.78", Some("+33612345678")),
			("Téléphone\nje ne sais pas", None),
			("Call me at 06 12 34 56 78", None),
		] {
			assert_eq!(labeled_phone(description).as_deref(), want, "{description:?}");
		}
	}

	#[test]
	fn the_consent_url() {
		let u = consent_url("cid.apps.googleusercontent.com", "http://127.0.0.1:8765/", "st", "ch").unwrap();
		for part in [
			"access_type=offline",
			"prompt=consent",
			"scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar.events.readonly",
			"code_challenge_method=S256",
			"redirect_uri=http%3A%2F%2F127.0.0.1%3A8765%2F",
		] {
			assert!(u.contains(part), "{part} in {u}");
		}
	}
}
