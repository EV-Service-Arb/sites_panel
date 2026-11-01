//! What the conductor's Telegram tells of the sites (docs/ARCHITECTURE.md, Notifications): the
//! fan-out finds what a rule is due for, claims it once and appends one note in the same
//! transaction; the conductor reads the notes after its cursor and decides who gets each.
//!
//! ```text
//! journal ─ fan-out (a rule × a new lead / overdue lead / payment / booking) ─ claim (rule, event) + note
//! GET /api/conductor/notifications?after ─ notes, texts opened
//! ```

use eyre::WrapErr;
use jiff::{SignedDuration, Timestamp};
use panel_core::{
	funnel::CONTACT_SLA,
	ids::{BrandId, LeadId},
	notify::{BookingChange, LeadNote, Locale, Note, Rendered, Rule},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
	Panel,
	seal::{note_aad, pii_aad},
	store::{
		self,
		notifications::{self as db, LeadCandidate, NewNote},
	},
};

/// A lead is told of as new while its creation is younger than this; older ones, when the
/// fan-out starts late, are left to the SLA reminder.
const NEW_LEAD_WINDOW: SignedDuration = SignedDuration::from_hours(1);

/// A lead is reminded of while it has been overdue for less than this.
const OVERDUE_WINDOW: SignedDuration = SignedDuration::from_hours(6);

/// A payment is told of while its record is younger than this.
const PAYMENT_WINDOW: SignedDuration = SignedDuration::from_hours(24);

/// A booking made, moved or canceled is told of while its event is younger than this.
const BOOKING_WINDOW: SignedDuration = SignedDuration::from_hours(6);

/// How long a note waits for the conductor.
const NOTE_KEEP: SignedDuration = SignedDuration::from_hours(24);

/// How long a claim is kept; every window above is shorter.
const CLAIM_KEEP: SignedDuration = SignedDuration::from_hours(24 * 7);

/// Candidates taken per rule per pass.
const BATCH: i64 = 50;

/// A note as the conductor reads it.
#[derive(Clone, Debug)]
pub struct Notification {
	pub seq: i64,
	pub rule: String,
	pub event_id: Uuid,
	pub created_at: Timestamp,
	pub text: Rendered,
	/// The text with the customer's PII, when it carries any.
	pub text_pii: Option<Rendered>,
	/// Who did what the note tells (typed the lead in, set the slot): they know of it already.
	pub skip_user: Option<Uuid>,
	/// The lead its buttons act on, `(brand, lead)`.
	pub lead: Option<(String, String)>,
	pub buttons: Vec<panel_core::notify::Button>,
}

#[derive(Deserialize, Serialize)]
struct SealedTexts {
	text: Plain,
	text_pii: Option<Plain>,
}

#[derive(Deserialize, Serialize)]
struct Plain {
	text: String,
	code: Vec<[usize; 2]>,
}

impl From<&Rendered> for Plain {
	fn from(r: &Rendered) -> Self {
		Self {
			text: r.text.clone(),
			code: r.code.iter().map(|e| [e.offset, e.length]).collect(),
		}
	}
}

impl From<Plain> for Rendered {
	fn from(p: Plain) -> Self {
		Self {
			text: p.text,
			code: p.code.into_iter().map(|[offset, length]| panel_core::notify::Entity { offset, length }).collect(),
		}
	}
}

/// A slot as the place reads it: Paris time (every place is in France, v1), or UTC, said so,
/// where the system has no time zone database.
fn place_time(at: Timestamp) -> String {
	match jiff::tz::TimeZone::get("Europe/Paris") {
		Ok(tz) => at.to_zoned(tz).strftime("%Y-%m-%d %H:%M (Paris)").to_string(),
		Err(_) => at.strftime("%Y-%m-%d %H:%M UTC").to_string(),
	}
}

fn text_field(pii: &Value, field: &str) -> Option<String> {
	pii.get(field).and_then(Value::as_str).map(str::to_owned)
}

impl Panel {
	/// Appends a note for what the rules say is due at `now`: new leads, overdue leads,
	/// payments and bookings, each once. Safe on several processes at once: each candidate is
	/// claimed with its note. Prunes notes past `NOTE_KEEP`. How many notes were appended.
	pub async fn fan_out(&self, now: Timestamp, locale: Locale) -> eyre::Result<usize> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the fan-out")?;
		db::prune(&mut conn, now - NOTE_KEEP, now - CLAIM_KEEP).await?;
		let new = db::new_leads(&mut conn, now - NEW_LEAD_WINDOW, BATCH).await?;
		let overdue = db::overdue_leads(&mut conn, now - CONTACT_SLA - OVERDUE_WINDOW, now - CONTACT_SLA, BATCH).await?;
		let payments = db::new_payments(&mut conn, now - PAYMENT_WINDOW, BATCH).await?;
		let mut appended = 0;
		for c in &new {
			let note = |pii: bool| {
				let lead = self.lead_note(c, pii)?;
				Ok(match c.suspect {
					None => Note::NewLead(lead),
					Some(suspect) => Note::SuspectLead { lead, suspect },
				})
			};
			let lead = Some((c.brand_id.as_str(), c.lead_id.as_str()));
			appended += self.append(&mut conn, Rule::NewLead, c.event_id, lead, c.entered_by, note, now, locale).await?;
		}
		for c in &overdue {
			let waiting = c.created_at.map_or(CONTACT_SLA, |at| now.duration_since(at));
			let note = |pii: bool| {
				Ok(Note::ContactOverdue {
					lead: self.lead_note(c, pii)?,
					waiting,
				})
			};
			let lead = Some((c.brand_id.as_str(), c.lead_id.as_str()));
			appended += self.append(&mut conn, Rule::ContactOverdue, c.event_id, lead, None, note, now, locale).await?;
		}
		for p in payments {
			let note = Note::PaymentReceived {
				brand: p.brand_id,
				lead: p.lead_id,
				billed: p.billed,
				commission: p.commission,
				currency: p.currency,
			};
			appended += self.append(&mut conn, Rule::PaymentReceived, p.event_id, None, None, |_| Ok(note.clone()), now, locale).await?;
		}
		for t in store::bookings::to_tell(&mut conn, now - BOOKING_WINDOW, BATCH).await? {
			let change = match (t.kind.as_str(), t.moved) {
				("created", true) => BookingChange::Moved,
				("created" | "set", _) => BookingChange::Booked,
				_ => BookingChange::Canceled,
			};
			let lead = match &t.lead_id {
				Some(l) => {
					let (brand, lead) = (BrandId::parse(&t.brand_id)?, LeadId::parse(l)?);
					store::reads::lead(&mut conn, &brand, &lead).await?
				}
				None => None,
			};
			let candidate = lead.map(|row| LeadCandidate {
				event_id: row.creation.as_ref().map_or(t.event_id, |c| c.event_id),
				pii: row.creation.and_then(|c| c.pii),
				brand_id: row.brand_id,
				lead_id: row.lead_id,
				location_id: row.location_id,
				created_at: row.created_at,
				entered_by: None,
				suspect: None,
			});
			let slot = t.start_at.map(place_time);
			let note = |pii: bool| {
				Ok(Note::Booking {
					change,
					brand: t.brand_id.clone(),
					lead: candidate.as_ref().map(|c| self.lead_note(c, pii)).transpose()?,
					slot: slot.clone(),
					provider: t.provider.clone().unwrap_or_else(|| "manual".to_owned()),
				})
			};
			let lead = candidate.as_ref().map(|c| (c.brand_id.as_str(), c.lead_id.as_str()));
			appended += self.append(&mut conn, Rule::Booked, t.event_id, lead, t.by, note, now, locale).await?;
		}
		Ok(appended)
	}

	/// One candidate: claimed, and its note appended, in one transaction, so a claim never
	/// stands without its note. `note(true)` is the note with the customer's PII.
	#[expect(clippy::too_many_arguments, reason = "the candidate and the pass's context, each named at the call site")]
	async fn append(
		&self,
		conn: &mut sqlx::SqliteConnection,
		rule: Rule,
		event: Uuid,
		lead: Option<(&str, &str)>,
		skip_user: Option<Uuid>,
		note: impl Fn(bool) -> eyre::Result<Note>,
		now: Timestamp,
		locale: Locale,
	) -> eyre::Result<usize> {
		let mut tx = store::begin_write(&mut *conn).await?;
		if !db::claim_fanout(&mut tx, rule, event, now).await? {
			return Ok(0);
		}
		let without = note(false)?;
		let text = without.render(locale);
		let with = note(true)?.render(locale);
		let texts = SealedTexts {
			text: Plain::from(&text),
			text_pii: (with != text).then(|| Plain::from(&with)),
		};
		let plain = Zeroizing::new(serde_json::to_vec(&texts).wrap_err("serializing a note")?);
		let sealed = self.key.seal(&note_aad(rule.as_str(), event), &plain)?;
		db::append(
			&mut tx,
			&NewNote {
				rule,
				event_id: event,
				lead,
				buttons: without.buttons().iter().map(|b| b.as_str()).collect(),
				skip_user,
				texts_sealed: &sealed,
				data_key_fp: self.key.fingerprint(),
			},
			now,
		)
		.await?;
		tx.commit().await.wrap_err("committing a note")?;
		tracing::info!(%rule, %event, "notification: appended");
		Ok(1)
	}

	/// The notes after `after`, oldest first, at most `limit`, their texts opened.
	pub async fn notifications(&self, after: i64, limit: i64) -> eyre::Result<Vec<Notification>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the notes")?;
		db::after(&mut conn, after, limit)
			.await?
			.into_iter()
			.map(|n| {
				eyre::ensure!(n.data_key_fp.as_slice() == self.key.fingerprint(), "note {} was sealed under another PANEL_DATA_KEY", n.seq);
				let plain = self
					.key
					.open(&note_aad(&n.rule, n.event_id), &n.texts_sealed)
					.wrap_err_with(|| format!("opening note {}", n.seq))?;
				let texts: SealedTexts = serde_json::from_slice(&plain).wrap_err("a stored note is not one")?;
				let buttons = n
					.buttons
					.iter()
					.map(|b| panel_core::notify::Button::from_name(b).ok_or_else(|| eyre::eyre!("note {} names no button {b:?}", n.seq)))
					.collect::<eyre::Result<_>>()?;
				Ok(Notification {
					seq: n.seq,
					rule: n.rule,
					event_id: n.event_id,
					created_at: n.created_at,
					text: texts.text.into(),
					text_pii: texts.text_pii.map(Into::into),
					skip_user: n.skip_user,
					lead: n.lead,
					buttons,
				})
			})
			.collect()
	}

	/// When each source key last had an event received.
	pub async fn received_by_key(&self) -> eyre::Result<Vec<(String, Timestamp)>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection for the receipts")?;
		db::received_by_key(&mut conn).await
	}

	/// A lead as a note shows it: with `pii`, what the customer left at its creation.
	fn lead_note(&self, c: &LeadCandidate, pii: bool) -> eyre::Result<LeadNote> {
		let mut note = LeadNote {
			brand: c.brand_id.clone(),
			location: c.location_id.clone(),
			..LeadNote::default()
		};
		if !pii {
			return Ok(note);
		}
		if let Some((blob, fp)) = &c.pii {
			eyre::ensure!(fp.as_slice() == self.key.fingerprint(), "the PII of event {} was sealed under another PANEL_DATA_KEY", c.event_id);
			let plain = self.key.open(&pii_aad(c.event_id), blob).wrap_err_with(|| format!("opening the PII of event {}", c.event_id))?;
			let pii: Value = serde_json::from_slice(&plain).wrap_err("stored PII is not JSON")?;
			note.name = text_field(&pii, "name");
			note.need = text_field(&pii, "need");
			note.phone = text_field(&pii, "phone");
		}
		Ok(note)
	}
}
