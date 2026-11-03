//! The notes the conductor's Telegram reads: the fan-out's candidates and claims, the notes
//! themselves, and what each source key last sent. Every timestamp is the caller's `now`.

use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::fact::LeadSuspect;
use sa_alerts::Rule;
use sqlx::{SqliteConnection, types::Json};
use uuid::Uuid;

use super::{from_db, to_db};

/// A lead's creation not yet fanned out for `rule`, with its sealed PII.
#[derive(Clone, Debug)]
pub struct LeadCandidate {
	pub event_id: Uuid,
	pub brand_id: String,
	pub lead_id: String,
	pub location_id: Option<String>,
	pub created_at: Option<Timestamp>,
	pub pii: Option<(Vec<u8>, Vec<u8>)>,
	/// The panel user who typed the lead in (its creation of kind `panel`), if one did.
	pub entered_by: Option<Uuid>,
	/// Why the landing's antispam doubted it; `None` for an ordinary lead.
	pub suspect: Option<LeadSuspect>,
}

type LeadCandidateDb = (
	Uuid,
	String,
	String,
	Option<String>,
	Option<i64>,
	Option<Vec<u8>>,
	Option<Vec<u8>>,
	Option<String>,
	Option<String>,
);

fn lead_candidate((event_id, brand_id, lead_id, location_id, created_at, pii, fp, entered_by, suspect): LeadCandidateDb) -> eyre::Result<LeadCandidate> {
	Ok(LeadCandidate {
		suspect: suspect
			.as_deref()
			.map(LeadSuspect::parse)
			.transpose()
			.wrap_err_with(|| format!("stored suspect mark of lead {brand_id}/{lead_id}"))?,
		event_id,
		brand_id,
		lead_id,
		location_id,
		created_at: created_at.map(from_db).transpose()?,
		pii: pii.zip(fp),
		// A panel event's source id is the user's concierge id; anything else names no one.
		entered_by: entered_by.and_then(|id| Uuid::parse_str(&id).ok()),
	})
}

/// Leads whose creation (the one that counts: the first journaled) arrived at or after
/// `since`, still waiting for their first contact, and not yet told of.
pub async fn new_leads(conn: &mut SqliteConnection, since: Timestamp, limit: i64) -> eyre::Result<Vec<LeadCandidate>> {
	let rows: Vec<LeadCandidateDb> = sqlx::query_as(
		"SELECT e.id, l.brand_id, l.lead_id, l.location_id, l.created_at, e.pii_sealed, e.data_key_fp, \
		 CASE WHEN e.source_kind = 'panel' THEN e.source_id END, l.suspect \
		 FROM events e JOIN leads l ON l.brand_id = e.brand_id AND l.lead_id = e.lead_id \
		 WHERE e.type = 'lead.created' AND e.status = 'registered' AND e.received_at >= $1 AND l.stage = 'created' \
		 AND NOT EXISTS (SELECT 1 FROM events f WHERE f.brand_id = e.brand_id AND f.lead_id = e.lead_id AND f.type = 'lead.created' \
		                 AND f.status = 'registered' AND (f.received_at, f.id) < (e.received_at, e.id)) \
		 AND NOT EXISTS (SELECT 1 FROM fanout t WHERE t.rule = 'new_lead' AND t.event_id = e.id) \
		 ORDER BY e.received_at, e.id LIMIT $2",
	)
	.bind(to_db(since))
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("finding new leads to tell of")?;
	rows.into_iter().map(lead_candidate).collect()
}

/// Leads created in `[since, before)` and still not contacted — past the SLA when `before`
/// is now minus it — not yet reminded of. Keyed by their creation event. A lead the antispam
/// doubted is told of once, as such, and never chased: its SLA is nobody's promise.
pub async fn overdue_leads(conn: &mut SqliteConnection, since: Timestamp, before: Timestamp, limit: i64) -> eyre::Result<Vec<LeadCandidate>> {
	let rows: Vec<LeadCandidateDb> = sqlx::query_as(
		"SELECT c.id, l.brand_id, l.lead_id, l.location_id, l.created_at, c.pii_sealed, c.data_key_fp, \
		 CASE WHEN c.source_kind = 'panel' THEN c.source_id END, l.suspect FROM leads l \
		 JOIN events c ON c.id = ( \
		   SELECT e.id FROM events e \
		   WHERE e.brand_id = l.brand_id AND e.lead_id = l.lead_id AND e.type = 'lead.created' AND e.status = 'registered' \
		   ORDER BY e.received_at, e.id LIMIT 1 \
		 ) \
		 WHERE l.stage = 'created' AND l.contacted_at IS NULL AND l.created_at >= $1 AND l.created_at < $2 AND l.suspect IS NULL \
		 AND NOT EXISTS (SELECT 1 FROM fanout t WHERE t.rule = 'contact_overdue' AND t.event_id = c.id) \
		 ORDER BY l.created_at LIMIT $3",
	)
	.bind(to_db(since))
	.bind(to_db(before))
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("finding overdue leads to remind of")?;
	rows.into_iter().map(lead_candidate).collect()
}

/// A payment not yet told of.
#[derive(Clone, Debug)]
pub struct PaymentCandidate {
	pub event_id: Uuid,
	pub brand_id: String,
	pub lead_id: String,
	pub billed: i64,
	pub commission: i64,
	pub currency: String,
}

/// Payments journaled at or after `since`, not yet told of.
pub async fn new_payments(conn: &mut SqliteConnection, since: Timestamp, limit: i64) -> eyre::Result<Vec<PaymentCandidate>> {
	let rows: Vec<(Uuid, String, String, i64, i64, String)> = sqlx::query_as(
		"SELECT p.event_id, p.brand_id, p.lead_id, p.billed, p.commission, p.currency FROM payments p JOIN events e ON e.id = p.event_id \
		 WHERE e.received_at >= $1 AND NOT EXISTS (SELECT 1 FROM fanout t WHERE t.rule = 'payment_received' AND t.event_id = p.event_id) \
		 ORDER BY e.received_at, e.id LIMIT $2",
	)
	.bind(to_db(since))
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("finding payments to tell of")?;
	Ok(rows
		.into_iter()
		.map(|(event_id, brand_id, lead_id, billed, commission, currency)| PaymentCandidate {
			event_id,
			brand_id,
			lead_id,
			billed,
			commission,
			currency,
		})
		.collect())
}

/// Marks `(rule, event_id)` as fanned out; `false` when it already was (by another replica).
pub async fn claim_fanout(conn: &mut SqliteConnection, rule: Rule, event_id: Uuid, now: Timestamp) -> eyre::Result<bool> {
	let done = sqlx::query("INSERT INTO fanout (rule, event_id, fanned_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
		.bind(rule.as_str())
		.bind(event_id)
		.bind(to_db(now))
		.execute(&mut *conn)
		.await
		.wrap_err("marking a fan-out")?
		.rows_affected();
	Ok(done == 1)
}

/// A note to append, its texts sealed.
pub struct NewNote<'a> {
	pub rule: Rule,
	pub event_id: Uuid,
	pub lead: Option<(&'a str, &'a str)>,
	pub buttons: Vec<&'static str>,
	pub skip_user: Option<Uuid>,
	pub texts_sealed: &'a [u8],
	pub data_key_fp: [u8; 32],
}

/// Appends a note: its `seq` is the next one, in commit order (one writer).
pub async fn append(conn: &mut SqliteConnection, n: &NewNote<'_>, now: Timestamp) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO notifications (rule, event_id, created_at, brand_id, lead_id, buttons, skip_user, texts_sealed, data_key_fp) \
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
	)
	.bind(n.rule.as_str())
	.bind(n.event_id)
	.bind(to_db(now))
	.bind(n.lead.map(|(b, _)| b))
	.bind(n.lead.map(|(_, l)| l))
	.bind(Json(&n.buttons))
	.bind(n.skip_user)
	.bind(n.texts_sealed)
	.bind(n.data_key_fp.as_slice())
	.execute(&mut *conn)
	.await
	.wrap_err("appending a note")?;
	Ok(())
}

/// A note as stored.
pub struct StoredNote {
	pub seq: i64,
	pub rule: String,
	pub event_id: Uuid,
	pub created_at: Timestamp,
	pub lead: Option<(String, String)>,
	pub buttons: Vec<String>,
	pub skip_user: Option<Uuid>,
	pub texts_sealed: Vec<u8>,
	pub data_key_fp: Vec<u8>,
}

/// The notes after `after`, oldest first, at most `limit`.
pub async fn after(conn: &mut SqliteConnection, after: i64, limit: i64) -> eyre::Result<Vec<StoredNote>> {
	type Row = (i64, String, Uuid, i64, Option<String>, Option<String>, Json<Vec<String>>, Option<Uuid>, Vec<u8>, Vec<u8>);
	let rows: Vec<Row> = sqlx::query_as(
		"SELECT seq, rule, event_id, created_at, brand_id, lead_id, buttons, skip_user, texts_sealed, data_key_fp \
		 FROM notifications WHERE seq > $1 ORDER BY seq LIMIT $2",
	)
	.bind(after)
	.bind(limit)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading the notes")?;
	rows.into_iter()
		.map(|(seq, rule, event_id, created_at, brand, lead, Json(buttons), skip_user, texts_sealed, data_key_fp)| {
			Ok(StoredNote {
				seq,
				rule,
				event_id,
				created_at: from_db(created_at)?,
				lead: brand.zip(lead),
				buttons,
				skip_user,
				texts_sealed,
				data_key_fp,
			})
		})
		.collect()
}

/// Drops notes created before `notes_before` and claims made before `claims_before`.
pub async fn prune(conn: &mut SqliteConnection, notes_before: Timestamp, claims_before: Timestamp) -> eyre::Result<()> {
	sqlx::query("DELETE FROM notifications WHERE created_at < $1")
		.bind(to_db(notes_before))
		.execute(&mut *conn)
		.await
		.wrap_err("pruning the notes")?;
	sqlx::query("DELETE FROM fanout WHERE fanned_at < $1")
		.bind(to_db(claims_before))
		.execute(&mut *conn)
		.await
		.wrap_err("pruning the fan-out claims")?;
	Ok(())
}

/// When each source key last had an event received: every key the journal names.
pub async fn received_by_key(conn: &mut SqliteConnection) -> eyre::Result<Vec<(String, Timestamp)>> {
	let rows: Vec<(String, i64)> = sqlx::query_as("SELECT key_id, max(received_at) FROM events WHERE key_id IS NOT NULL GROUP BY key_id ORDER BY key_id")
		.fetch_all(&mut *conn)
		.await
		.wrap_err("reading each key's last receipt")?;
	rows.into_iter().map(|(k, at)| Ok((k, from_db(at)?))).collect()
}
