//! The places' live settings: `places` (registered, withdrawn), `place_settings` (the current
//! settings) and `place_changes` (their append-only history). Writes take a transaction from
//! [`super::begin_write`], so a read of the current settings and the write after it are one.

use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::{
	ids::{BrandId, LocationId},
	place::{Change, ChangeKind, PlaceSettings},
};
use serde_json::Value;
use sqlx::{SqliteConnection, types::Json};
use uuid::Uuid;

use super::{from_db, reads, to_db};

/// A place as stored: whether it is registered at all, withdrawn, and its settings if ever set.
#[derive(Clone, Debug, Default)]
pub struct StoredPlace {
	pub registered: bool,
	pub withdrawn: bool,
	pub gmb_id: Option<i64>,
	pub settings: Option<StoredSettings>,
}

#[derive(Clone, Debug)]
pub struct StoredSettings {
	pub settings: PlaceSettings,
	pub updated_at: Timestamp,
	pub updated_by: String,
}

/// Who wrote, as the rows keep it: the label shown, and the user id when a user.
#[derive(Clone, Copy, Debug)]
pub struct By<'a> {
	pub label: &'a str,
	pub user: Option<Uuid>,
}

fn settings_of(raw: Value, what: &str) -> eyre::Result<PlaceSettings> {
	PlaceSettings::stored(raw).ok_or_else(|| eyre::eyre!("stored {what} is not a JSON object"))
}

pub async fn place(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId) -> eyre::Result<StoredPlace> {
	/// `withdrawn`, `gmb_id`, `settings`, `updated_at`, `updated_by`.
	type Row = (bool, Option<i64>, Option<Json<Value>>, Option<i64>, Option<String>);
	let row: Option<Row> = sqlx::query_as(
		"SELECT p.withdrawn, p.gmb_id, s.settings, s.updated_at, s.updated_by FROM places p \
		 LEFT JOIN place_settings s ON s.brand_id = p.brand_id AND s.location_id = p.location_id \
		 WHERE p.brand_id = $1 AND p.location_id = $2",
	)
	.bind(brand.as_str())
	.bind(slug.as_str())
	.fetch_optional(&mut *conn)
	.await
	.wrap_err("reading a place")?;
	let Some((withdrawn, gmb_id, settings, updated_at, updated_by)) = row else {
		return Ok(StoredPlace::default());
	};
	let settings = match (settings, updated_at, updated_by) {
		(Some(Json(s)), Some(at), Some(by)) => Some(StoredSettings {
			settings: settings_of(s, "place settings")?,
			updated_at: from_db(at)?,
			updated_by: by,
		}),
		_ => None,
	};
	Ok(StoredPlace {
		registered: true,
		withdrawn,
		gmb_id,
		settings,
	})
}

/// Registers a place unless it is; `true` when this did.
pub async fn register(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId, at: Timestamp, by: By<'_>) -> eyre::Result<bool> {
	let done = sqlx::query("INSERT INTO places (brand_id, location_id, registered_at, registered_by) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
		.bind(brand.as_str())
		.bind(slug.as_str())
		.bind(to_db(at))
		.bind(by.label)
		.execute(&mut *conn)
		.await
		.wrap_err("registering a place")?;
	Ok(done.rows_affected() == 1)
}

pub async fn write_settings(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId, settings: &PlaceSettings, at: Timestamp, by: By<'_>) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO place_settings (brand_id, location_id, settings, updated_at, updated_by, updated_by_user) VALUES ($1, $2, $3, $4, $5, $6) \
		 ON CONFLICT (brand_id, location_id) DO UPDATE SET settings = EXCLUDED.settings, updated_at = EXCLUDED.updated_at, \
		 updated_by = EXCLUDED.updated_by, updated_by_user = EXCLUDED.updated_by_user",
	)
	.bind(brand.as_str())
	.bind(slug.as_str())
	.bind(Json(settings.as_json()))
	.bind(to_db(at))
	.bind(by.label)
	.bind(by.user)
	.execute(&mut *conn)
	.await
	.wrap_err("writing a place's settings")?;
	Ok(())
}

pub async fn set_withdrawn(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId, withdrawn: bool) -> eyre::Result<()> {
	sqlx::query("UPDATE places SET withdrawn = $3 WHERE brand_id = $1 AND location_id = $2")
		.bind(brand.as_str())
		.bind(slug.as_str())
		.bind(withdrawn)
		.execute(&mut *conn)
		.await
		.wrap_err("withdrawing or restoring a place")?;
	Ok(())
}

/// Links a place to a GMB (or frees it with `None`); `false` when another place has that GMB.
pub async fn set_gmb(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId, gmb_id: Option<i64>) -> eyre::Result<bool> {
	let taken: Option<i64> = match gmb_id {
		Some(g) => sqlx::query_scalar("SELECT 1 FROM places WHERE gmb_id = $1 AND NOT (brand_id = $2 AND location_id = $3)")
			.bind(g)
			.bind(brand.as_str())
			.bind(slug.as_str())
			.fetch_optional(&mut *conn)
			.await
			.wrap_err("looking for the place of a GMB")?,
		None => None,
	};
	if taken.is_some() {
		return Ok(false);
	}
	sqlx::query("UPDATE places SET gmb_id = $3 WHERE brand_id = $1 AND location_id = $2")
		.bind(brand.as_str())
		.bind(slug.as_str())
		.bind(gmb_id)
		.execute(&mut *conn)
		.await
		.wrap_err("linking a place to a GMB")?;
	Ok(true)
}

/// Per linked place: `(gmb_id, brand, slug, leads created since `since`, leads waiting for their first contact since before `waiting_since_before`)`.
pub async fn gmbs(conn: &mut SqliteConnection, since: Timestamp, waiting_since_before: Timestamp) -> eyre::Result<Vec<(i64, String, String, u32, u32)>> {
	let rows: Vec<(i64, String, String, i64, i64)> = sqlx::query_as(concat!(
		"SELECT p.gmb_id, p.brand_id, p.location_id, count(l.lead_id) FILTER (WHERE l.created_at >= $1), count(l.lead_id) FILTER (WHERE ",
		reads::overdue!(),
		"$2) FROM places p LEFT JOIN leads l ON l.brand_id = p.brand_id AND l.location_id = p.location_id \
		 WHERE p.gmb_id IS NOT NULL GROUP BY p.gmb_id, p.brand_id, p.location_id ORDER BY p.gmb_id"
	))
	.bind(to_db(since))
	.bind(to_db(waiting_since_before))
	.fetch_all(&mut *conn)
	.await
	.wrap_err("listing the GMBs' places")?;
	rows.into_iter()
		.map(|(id, brand, slug, recent, overdue)| Ok((id, brand, slug, u32::try_from(recent)?, u32::try_from(overdue)?)))
		.collect()
}

pub async fn insert_change(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId, change: &Change, by_user: Option<Uuid>) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO place_changes (id, brand_id, location_id, at, kind, changed_by, changed_by_user, before, after, reverts) \
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
	)
	.bind(change.id)
	.bind(brand.as_str())
	.bind(slug.as_str())
	.bind(to_db(change.at))
	.bind(change.kind.as_str())
	.bind(&change.by)
	.bind(by_user)
	.bind(Json(change.before.as_json()))
	.bind(Json(change.after.as_json()))
	.bind(change.reverts)
	.execute(&mut *conn)
	.await
	.wrap_err("journaling a place's change")?;
	Ok(())
}

#[derive(sqlx::FromRow)]
struct ChangeRow {
	id: Uuid,
	at: i64,
	kind: String,
	changed_by: String,
	before: Json<Value>,
	after: Json<Value>,
	reverts: Option<Uuid>,
}

impl TryFrom<ChangeRow> for Change {
	type Error = eyre::Report;

	fn try_from(r: ChangeRow) -> eyre::Result<Self> {
		Ok(Self {
			id: r.id,
			at: from_db(r.at)?,
			kind: ChangeKind::parse(&r.kind).ok_or_else(|| eyre::eyre!("stored change kind {:?}", r.kind))?,
			by: r.changed_by,
			before: settings_of(r.before.0, "settings before a change")?,
			after: settings_of(r.after.0, "settings after a change")?,
			reverts: r.reverts,
		})
	}
}

/// A place's changes, newest first. Two in the same microsecond go by insertion order
/// (`rowid`): their ids, UUIDv7 with random low bits, do not order within one.
pub async fn changes(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId) -> eyre::Result<Vec<Change>> {
	let rows: Vec<ChangeRow> = sqlx::query_as(
		"SELECT id, at, kind, changed_by, before, after, reverts FROM place_changes \
		 WHERE brand_id = $1 AND location_id = $2 ORDER BY at DESC, rowid DESC",
	)
	.bind(brand.as_str())
	.bind(slug.as_str())
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading a place's history")?;
	rows.into_iter().map(Change::try_from).collect()
}

/// One change of this place; `None` when it is not one of its.
pub async fn change(conn: &mut SqliteConnection, brand: &BrandId, slug: &LocationId, id: Uuid) -> eyre::Result<Option<Change>> {
	let row: Option<ChangeRow> = sqlx::query_as(
		"SELECT id, at, kind, changed_by, before, after, reverts FROM place_changes \
		 WHERE brand_id = $1 AND location_id = $2 AND id = $3",
	)
	.bind(brand.as_str())
	.bind(slug.as_str())
	.bind(id)
	.fetch_optional(&mut *conn)
	.await
	.wrap_err("reading a place's change")?;
	row.map(Change::try_from).transpose()
}
