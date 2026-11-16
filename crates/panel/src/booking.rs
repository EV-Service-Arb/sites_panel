//! Booking (FORM-VARIANTS-SPEC, "Booking providers contract"): what an operator does to a
//! lead's slot, the providers' seam, and how a provider's booking finds its lead.
//!
//! ```text
//! site      booking.requested (ingest, signed)                          ┐
//! operator  booking.set | status_changed | cleared | attached (panel)   ├─ journal ─ bookings, leads.booking_*
//! provider  push: webhook ─ PushSource::verify ─┐                      │
//!           pull: PullSource::pull (leased) ────┴─ BookingEvent ─ match ┘ booking.created | canceled (kind booking)
//! ```
//!
//! **The seam.** Every provider, push or pull, hands the engine normalized
//! [`BookingEvent`]s; [`Panel::ingest_bookings`] matches and journals them. A provider's
//! booking is keyed by `(brand, provider, external_ref)`; its journal event's id derives from
//! that key, the change and the provider's revision, so the same revision seen twice — a
//! retried webhook, a pull repeated after a crash — is a duplicate.
//!
//! **Matching**, once per booking (a later event of it keeps the lead it found): the ref the
//! page carried back, when it names a lead of the brand (`match: ref`); else the attendee's
//! phone (E.164, the sites' rule) or email against the PII of the brand's leads created in
//! the [`MATCH_WINDOW`] before the booking — exactly one lead is a match (`contact`), none or
//! several leave it unmatched for an operator to attach (`manual`). The leads' PII is opened
//! under `PANEL_DATA_KEY` only to compare, and dropped.

use std::{collections::BTreeMap, future::Future, sync::Arc};

use eyre::WrapErr;
use jiff::{SignedDuration, Timestamp};
use panel_contracts::SCHEMA;
pub use panel_core::booking::{BookingEvent, Change, Contact, Provider};
use panel_core::{
	booking::{BookingMatch, BookingStatus, Closed, OperatorAction, is_external_ref, is_version},
	ids::{EventId, LeadId},
	phone,
};
use sa_core::ids::BrandId;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
	Outcome, Panel,
	operator::{ActionError, Actor, Conflict, Done, Pii, idempotent_id, subject_of},
	store::{
		bookings::{self, BookingRow},
		reads::Sealed,
	},
};

/// A booking is matched to a lead created at most this long before it.
pub const MATCH_WINDOW: SignedDuration = SignedDuration::from_hours(24 * 14);

/// …or this little after it: the site journals its lead before the visitor books, but clocks
/// and outboxes do not agree to the second.
pub const MATCH_GRACE: SignedDuration = SignedDuration::from_mins(10);

/// How long a pull may hold a brand's sync.
pub const SYNC_LEASE: SignedDuration = SignedDuration::from_mins(5);

/// A failed pull is tried again after this long.
pub const SYNC_RETRY: SignedDuration = SignedDuration::from_mins(1);

// ── the providers' seam ─────────────────────────────────────────────────────────────────

/// A provider's webhook, as it arrived: the brand of its route, its headers (names
/// lowercase) and its body, untouched — the signature is over it.
#[derive(Clone, Copy, Debug)]
pub struct PushRequest<'a> {
	pub brand: &'a BrandId,
	pub headers: &'a [(String, String)],
	pub body: &'a [u8],
	pub now: Timestamp,
}

impl PushRequest<'_> {
	/// A header's value; `name` lowercase.
	pub fn header(&self, name: &str) -> Option<&str> {
		self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
	}
}

/// Why a webhook was refused.
#[derive(Debug, thiserror::Error)]
pub enum PushError {
	/// The signature, its window or the brand's secret: the caller is told no more.
	#[error("unauthorized")]
	Unauthorized,
	/// Signed, and not a body the adapter can read.
	#[error("{0}")]
	BadRequest(String),
}

/// A provider that tells the panel of its bookings (`POST /api/hooks/booking/{provider}/{brand}`):
/// checks a request is the provider's, and reads it as booking events.
pub trait PushSource: Send + Sync + 'static {
	fn provider(&self) -> Provider;
	fn verify(&self, request: &PushRequest<'_>) -> Result<Vec<BookingEvent>, PushError>;
}

/// The push providers the server answers for; any other is a 404.
#[derive(Clone, Default)]
pub struct PushSources(Arc<BTreeMap<Provider, Arc<dyn PushSource>>>);

impl PushSources {
	/// These, and `source` for its provider.
	pub fn with(self, source: impl PushSource) -> Self {
		let mut map: BTreeMap<Provider, Arc<dyn PushSource>> = (*self.0).clone();
		map.insert(source.provider(), Arc::new(source));
		Self(Arc::new(map))
	}

	pub fn get(&self, provider: Provider) -> Option<&dyn PushSource> {
		self.0.get(&provider).map(|s| &**s)
	}
}

impl std::fmt::Debug for PushSources {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_list().entries(self.0.keys()).finish()
	}
}

/// What a pull found, and where the next one starts.
#[derive(Clone, Debug, Default)]
pub struct Pulled {
	pub events: Vec<BookingEvent>,
	/// The provider's cursor after these; `None`: the next pull is a full one.
	pub cursor: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PullError {
	/// The provider no longer knows the cursor (Google's 410 Gone): pull everything again.
	#[error("the provider's cursor expired")]
	CursorExpired,
	#[error(transparent)]
	Failed(#[from] eyre::Report),
}

/// A provider the panel asks for its bookings (a calendar without webhooks).
pub trait PullSource: Sync {
	fn provider(&self) -> Provider;
	/// The bookings changed since `cursor`; everything recent when `None`.
	fn pull(&self, brand: &BrandId, cursor: Option<&str>, now: Timestamp) -> impl Future<Output = Result<Pulled, PullError>> + Send;
}

/// What journaling a provider's events did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Ingested {
	/// Journaled now.
	pub written: u64,
	/// Of those, joined to a lead, and not.
	pub matched: u64,
	pub unmatched: u64,
	/// Journaled before: the same revision again.
	pub duplicate: u64,
	/// The booking as it stands already (the same slot; canceled twice).
	pub unchanged: u64,
	/// About a booking the panel never had (a cancellation of an ordinary event).
	pub ignored: u64,
	/// Refused: a malformed ref or revision, or the registry said no; logged.
	pub refused: u64,
}

/// One pull.
#[derive(Clone, Debug, Default)]
pub struct Synced {
	/// From nothing: the first, after a 410, or asked for.
	pub full: bool,
	pub ingested: Ingested,
}

/// A provider's booking as the screens show it, with its attendee for whoever sees PII.
#[derive(Clone, Debug)]
pub struct BookingView {
	pub row: BookingRow,
	pub contact: Option<Value>,
}

/// What an operator does to a lead's slot.
#[derive(Clone, Debug)]
pub enum SlotAction {
	/// Booked for this slot (RFC 3339 instants, as the API got them).
	Set {
		start_at: String,
		end_at: Option<String>,
	},
	Clear,
}

fn conflict(status: BookingStatus, action: OperatorAction) -> ActionError {
	ActionError::Conflict(match (action, status) {
		(OperatorAction::Clear, BookingStatus::None) => Conflict::NoBooking,
		(OperatorAction::Set | OperatorAction::Clear, _) => Conflict::BookingDone,
		(OperatorAction::Close(_), _) => Conflict::NotBooked,
	})
}

impl Panel {
	/// Journals what a provider says of a brand's bookings: each event matched to a lead,
	/// once, and written as `booking.created` / `booking.canceled` (kind booking), its
	/// attendee's PII sealed.
	pub async fn ingest_bookings(&self, brand: &BrandId, events: Vec<BookingEvent>, now: Timestamp) -> eyre::Result<Ingested> {
		let mut done = Ingested::default();
		for ev in events {
			if !ev.provider.has_adapter() || !is_external_ref(&ev.external_ref) || !is_version(&ev.version) {
				tracing::warn!(%brand, provider = %ev.provider, "booking: an adapter's event with a malformed ref or revision, left out");
				done.refused += 1;
				continue;
			}
			let existing = {
				let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
				bookings::by_key(&mut conn, brand, ev.provider, &ev.external_ref).await?
			};
			let (r#type, lead) = match &ev.change {
				Change::Canceled => {
					let Some(b) = existing else {
						done.ignored += 1;
						continue;
					};
					if !b.booked {
						done.unchanged += 1;
						continue;
					}
					("booking.canceled", b.lead)
				}
				Change::Booked { start, end, booked_at } => {
					if let Some(b) = &existing
						&& b.booked && b.start_at == *start
						&& b.end_at == *end
					{
						done.unchanged += 1;
						continue;
					}
					let lead = match existing.and_then(|b| b.lead) {
						Some(lead) => Some(lead),
						None => self.find_lead(brand, &ev, booked_at.unwrap_or(ev.at)).await?,
					};
					("booking.created", lead)
				}
			};
			let id = crate::derived_id(
				b"sa-panel/booking-event/v1/",
				&[
					ev.provider.as_str().as_bytes(),
					brand.as_str().as_bytes(),
					ev.external_ref.as_bytes(),
					r#type.as_bytes(),
					ev.version.as_bytes(),
				],
			);
			let mut properties = json!({"provider": ev.provider.as_str(), "externalRef": ev.external_ref, "version": ev.version});
			let mut subject = json!({"brandId": brand.as_str()});
			if let Change::Booked { start, end, booked_at } = &ev.change {
				properties["startAt"] = json!(start.to_string());
				if let Some(end) = end {
					properties["endAt"] = json!(end.to_string());
				}
				if let Some(at) = booked_at {
					properties["bookedAt"] = json!(at.to_string());
				}
				if let Some((lead, how)) = &lead {
					properties["match"] = json!(how.as_str());
					subject["leadId"] = json!(lead.as_str());
				}
			} else if let Some((lead, _)) = &lead {
				subject["leadId"] = json!(lead.as_str());
			}
			let mut raw = json!({
				"id": id.to_string(),
				"schema": SCHEMA,
				"type": r#type,
				"typeVersion": 1,
				"occurredAt": ev.at.min(now).to_string(),
				"source": {"kind": "booking", "id": ev.provider.as_str()},
				"subject": subject,
				"properties": properties,
			});
			let pii = contact_pii(&ev.contact);
			if !pii.is_empty() {
				raw["pii"] = Value::Object(pii);
			}
			match self.write_own(raw, now).await? {
				Ok((Outcome::Accepted { .. }, _)) => {
					done.written += 1;
					if lead.is_some() { done.matched += 1 } else { done.unmatched += 1 }
				}
				Ok((Outcome::Duplicate, _)) => done.duplicate += 1,
				Ok((Outcome::Rejected(e) | Outcome::Deferred(e), _)) | Err(e) => {
					tracing::warn!(%brand, provider = %ev.provider, reason = %e, "booking: an adapter's event refused");
					done.refused += 1;
				}
			}
		}
		if done.written + done.refused > 0 {
			tracing::info!(%brand, written = done.written, matched = done.matched, unmatched = done.unmatched, refused = done.refused, "bookings journaled");
		}
		Ok(done)
	}

	/// The lead a new booking is for: the ref it carried, else one lead by contact, else none.
	async fn find_lead(&self, brand: &BrandId, ev: &BookingEvent, booked_at: Timestamp) -> eyre::Result<Option<(LeadId, BookingMatch)>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		if let Some(lead) = ev.lead_ref.as_deref().and_then(|r| LeadId::parse(r).ok())
			&& bookings::lead_exists(&mut conn, brand, &lead).await?
		{
			return Ok(Some((lead, BookingMatch::Ref)));
		}
		let Contact { email, phone: tel, .. } = &ev.contact;
		if email.is_none() && tel.is_none() {
			return Ok(None);
		}
		let candidates = bookings::candidates(&mut conn, brand, booked_at - MATCH_WINDOW, booked_at + MATCH_GRACE).await?;
		drop(conn);
		let mut found: Vec<LeadId> = Vec::new();
		for c in candidates {
			let Some(pii) = self.open_pii(&Sealed { event_id: c.creation, pii: c.pii })? else { continue };
			let field = |k: &str| pii.get(k).and_then(Value::as_str);
			let by_phone = tel.is_some() && field("phone").and_then(phone::normalize).as_deref() == tel.as_deref();
			let by_email = email.is_some() && field("email").map(|e| e.trim().to_lowercase()).as_deref() == email.as_deref();
			if (by_phone || by_email) && !found.contains(&c.lead_id) {
				found.push(c.lead_id);
			}
		}
		match found.len() {
			1 => Ok(found.pop().map(|l| (l, BookingMatch::Contact))),
			0 => Ok(None),
			n => {
				tracing::info!(%brand, candidates = n, "booking: the attendee's contact names several leads; left for an operator");
				Ok(None)
			}
		}
	}

	/// One pull of a brand's bookings from `source`, under the brand's lease: `None` when
	/// another process holds it or none is due (`every` since the last; `force`: whenever no
	/// one holds it). `full`: from nothing, whatever the cursor. A cursor the provider no
	/// longer knows is dropped and the pull made full; the cursor moves only once what it
	/// covers is journaled.
	#[expect(clippy::too_many_arguments, reason = "the brand, the lease's holder and the schedule, each named at the call site")]
	pub async fn sync_bookings<S: PullSource>(
		&self,
		source: &S,
		brand: &BrandId,
		holder: Uuid,
		now: Timestamp,
		every: SignedDuration,
		force: bool,
		full: bool,
	) -> eyre::Result<Option<Synced>> {
		let provider = source.provider();
		let (due, retry) = if force { (now, now) } else { (now - every, now - SYNC_RETRY) };
		let lease = {
			let mut conn = self.store.pool().acquire().await.wrap_err("a connection for a booking sync")?;
			bookings::sync_lease(&mut conn, provider, brand, holder, now, now + SYNC_LEASE, due, retry).await?
		};
		let Some(cursor) = lease else {
			eyre::ensure!(!force, "another process is syncing {brand}'s {provider} bookings; try again in a few minutes");
			return Ok(None);
		};
		let cursor = if full { None } else { cursor };
		let pulled = match source.pull(brand, cursor.as_deref(), now).await {
			Ok(p) => Ok((p, cursor.is_none())),
			Err(PullError::CursorExpired) if cursor.is_some() => {
				tracing::warn!(%brand, %provider, "booking sync: the cursor expired; pulling everything again");
				source
					.pull(brand, None, now)
					.await
					.map(|p| (p, true))
					.map_err(|e| eyre::eyre!("a full pull of {brand}'s {provider} bookings: {e}"))
			}
			Err(e) => Err(eyre::eyre!("pulling {brand}'s {provider} bookings: {e}")),
		};
		let synced = match pulled {
			Ok((p, full)) => self.ingest_bookings(brand, p.events, now).await.map(|ingested| (Synced { full, ingested }, p.cursor)),
			Err(e) => Err(e),
		};
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for a booking sync")?;
		match synced {
			Ok((synced, cursor)) => {
				bookings::sync_release(&mut conn, provider, brand, holder, Timestamp::now(), Some(cursor.as_deref())).await?;
				Ok(Some(synced))
			}
			Err(e) => {
				bookings::sync_release(&mut conn, provider, brand, holder, Timestamp::now(), None).await?;
				Err(e)
			}
		}
	}

	/// Sets or clears a lead's slot (`booking.set`, `booking.cleared`), at most once per
	/// idempotency `key` of the user and lead. 409 where the booking stands does not allow it.
	pub async fn book_once(&self, by: Actor, brand: &BrandId, lead: &LeadId, action: SlotAction, now: Timestamp, key: Option<&str>) -> Result<Done<EventId>, ActionError> {
		let id = key.map(|k| idempotent_id(by, &format!("booking/{brand}/{lead}"), k));
		if let Some(done) = self.replayed(by, id).await? {
			return Ok(done.map(|(_, e)| e));
		}
		let current = self.lead_row(brand, lead).await?.ok_or(ActionError::NotFound)?;
		let (op, r#type, properties) = match action {
			SlotAction::Set { start_at, end_at } => {
				let mut p = json!({ "startAt": start_at });
				if let Some(end) = end_at {
					p["endAt"] = json!(end);
				}
				(OperatorAction::Set, "booking.set", p)
			}
			SlotAction::Clear => (OperatorAction::Clear, "booking.cleared", json!({})),
		};
		if !current.booking.status.allows(op) {
			return Err(conflict(current.booking.status, op));
		}
		Ok(self.act(by, id, r#type, subject_of(&current), properties, None, now).await?.map(|(_, e)| e))
	}

	/// Closes a lead's booked slot: done, no_show or canceled (`booking.status_changed`).
	pub async fn close_booking_once(&self, by: Actor, brand: &BrandId, lead: &LeadId, status: &str, now: Timestamp, key: Option<&str>) -> Result<Done<EventId>, ActionError> {
		let closed = Closed::parse(status).map_err(ActionError::Invalid)?;
		let id = key.map(|k| idempotent_id(by, &format!("booking_status/{brand}/{lead}"), k));
		if let Some(done) = self.replayed(by, id).await? {
			return Ok(done.map(|(_, e)| e));
		}
		let current = self.lead_row(brand, lead).await?.ok_or(ActionError::NotFound)?;
		let op = OperatorAction::Close(closed);
		if !current.booking.status.allows(op) {
			return Err(conflict(current.booking.status, op));
		}
		let properties = json!({ "status": closed.as_str() });
		Ok(self.act(by, id, "booking.status_changed", subject_of(&current), properties, None, now).await?.map(|(_, e)| e))
	}

	/// Joins a provider's booking to a lead of its brand (`booking.attached`): one without a
	/// lead, or one matched by contact that the operator confirms or corrects.
	pub async fn attach_booking(&self, by: Actor, booking: Uuid, lead: &LeadId, now: Timestamp) -> Result<EventId, ActionError> {
		let row = {
			let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
			bookings::by_id(&mut conn, booking).await?.ok_or(ActionError::NotFound)?
		};
		let current = self.lead_row(&row.brand_id, lead).await?.ok_or(ActionError::NotFound)?;
		if row.lead.as_ref().is_some_and(|(l, m)| l == lead && *m == BookingMatch::Manual) {
			return Err(Conflict::AlreadyAttached.into());
		}
		let properties = json!({ "provider": row.provider.as_str(), "externalRef": row.external_ref });
		Ok(self.act(by, None, "booking.attached", subject_of(&current), properties, None, now).await?.value.1)
	}

	/// The providers' bookings no lead was found for, the next slot first, with the attendee
	/// for whoever sees PII.
	pub async fn unmatched_bookings(&self, brand: Option<&BrandId>, pii: Pii, limit: u32) -> eyre::Result<Vec<BookingView>> {
		let rows = {
			let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
			bookings::unmatched(&mut conn, brand, i64::from(limit.clamp(1, crate::operator::MAX_PAGE))).await?
		};
		let mut out = Vec::with_capacity(rows.len());
		for row in rows {
			let contact = match pii {
				Pii::Reveal => self.pii(row.created_event_id).await?,
				Pii::Withhold => None,
			};
			out.push(BookingView { row, contact });
		}
		Ok(out)
	}
}

/// The attendee as the journal seals it: only what the provider gave.
fn contact_pii(c: &Contact) -> Map<String, Value> {
	let mut m = Map::new();
	for (k, v) in [("name", &c.name), ("email", &c.email), ("phone", &c.phone)] {
		if let Some(v) = v {
			m.insert(k.to_owned(), json!(v));
		}
	}
	m
}
