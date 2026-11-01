//! The brands' pricing: `pricing` (the current model, NULL once removed), `pricing_changes`
//! (its append-only history) and `brand_locales` (the languages its labels must be in). Writes
//! take a transaction from [`super::begin_write`], so a read of the current model and the write
//! after it are one.

use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::ids::BrandId;
use serde_json::Value;
use sqlx::{SqliteConnection, types::Json};
use uuid::Uuid;

use super::{from_db, places::By, to_db};

/// A brand's pricing as stored: its locales if ever set, and its model's row if ever written.
#[derive(Clone, Debug, Default)]
pub struct StoredPricing {
	pub locales: Option<Vec<String>>,
	pub current: Option<StoredModel>,
}

#[derive(Clone, Debug)]
pub struct StoredModel {
	/// `None` once removed.
	pub model: Option<Value>,
	pub updated_at: Timestamp,
	pub updated_by: String,
}

/// `locales`, `model`, `updated_at`, `updated_by`.
type Row = (Option<Json<Vec<String>>>, Option<Json<Value>>, Option<i64>, Option<String>);

fn stored((locales, model, updated_at, updated_by): Row) -> eyre::Result<StoredPricing> {
	let current = match (updated_at, updated_by) {
		(Some(at), Some(by)) => Some(StoredModel {
			model: model.map(|Json(m)| m),
			updated_at: from_db(at)?,
			updated_by: by,
		}),
		_ => None,
	};
	Ok(StoredPricing {
		locales: locales.map(|Json(l)| l),
		current,
	})
}

pub async fn pricing(conn: &mut SqliteConnection, brand: &BrandId) -> eyre::Result<StoredPricing> {
	let row: Row = sqlx::query_as(
		"SELECT l.locales, p.model, p.updated_at, p.updated_by FROM (SELECT $1 AS brand_id) k \
		 LEFT JOIN brand_locales l ON l.brand_id = k.brand_id LEFT JOIN pricing p ON p.brand_id = k.brand_id",
	)
	.bind(brand.as_str())
	.fetch_one(&mut *conn)
	.await
	.wrap_err("reading a brand's pricing")?;
	stored(row)
}

/// Every brand the panel knows — named by a lead, a place or a landing's experiments, or
/// given pricing or locales — with its pricing, by brand. A stored id that is not a brand id
/// (none should be: every writer checks) is left out rather than failing the list.
pub async fn all(conn: &mut SqliteConnection) -> eyre::Result<Vec<(BrandId, StoredPricing)>> {
	/// The brand, then a [`Row`].
	type BrandRow = (String, Option<Json<Vec<String>>>, Option<Json<Value>>, Option<i64>, Option<String>);
	let rows: Vec<BrandRow> = sqlx::query_as(
		"WITH known (brand_id) AS ( \
		   SELECT brand_id FROM leads \
		   UNION SELECT brand_id FROM places \
		   UNION SELECT brand_id FROM experiments \
		   UNION SELECT brand_id FROM pricing \
		   UNION SELECT brand_id FROM brand_locales) \
		 SELECT k.brand_id, l.locales, p.model, p.updated_at, p.updated_by FROM known k \
		 LEFT JOIN brand_locales l ON l.brand_id = k.brand_id LEFT JOIN pricing p ON p.brand_id = k.brand_id \
		 ORDER BY k.brand_id",
	)
	.fetch_all(&mut *conn)
	.await
	.wrap_err("listing the brands' pricing")?;
	let mut out = Vec::with_capacity(rows.len());
	for (brand, locales, model, at, by) in rows {
		let Ok(id) = BrandId::parse(&brand) else {
			tracing::warn!(brand, "a stored brand id that is not one: left out of the pricing list");
			continue;
		};
		out.push((id, stored((locales, model, at, by))?));
	}
	Ok(out)
}

/// Writes the current model (`None`: removed).
pub async fn write(conn: &mut SqliteConnection, brand: &BrandId, model: Option<&Value>, at: Timestamp, by: By<'_>) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO pricing (brand_id, model, updated_at, updated_by, updated_by_user) VALUES ($1, $2, $3, $4, $5) \
		 ON CONFLICT (brand_id) DO UPDATE SET model = EXCLUDED.model, updated_at = EXCLUDED.updated_at, \
		 updated_by = EXCLUDED.updated_by, updated_by_user = EXCLUDED.updated_by_user",
	)
	.bind(brand.as_str())
	.bind(model.map(Json))
	.bind(to_db(at))
	.bind(by.label)
	.bind(by.user)
	.execute(&mut *conn)
	.await
	.wrap_err("writing a brand's pricing")?;
	Ok(())
}

pub async fn set_locales(conn: &mut SqliteConnection, brand: &BrandId, locales: &[String], at: Timestamp, by: By<'_>) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO brand_locales (brand_id, locales, updated_at, updated_by) VALUES ($1, $2, $3, $4) \
		 ON CONFLICT (brand_id) DO UPDATE SET locales = EXCLUDED.locales, updated_at = EXCLUDED.updated_at, updated_by = EXCLUDED.updated_by",
	)
	.bind(brand.as_str())
	.bind(Json(locales))
	.bind(to_db(at))
	.bind(by.label)
	.execute(&mut *conn)
	.await
	.wrap_err("writing a brand's locales")?;
	Ok(())
}

/// One entry of a brand's pricing history.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeRecord {
	pub id: Uuid,
	pub at: Timestamp,
	/// `set` | `remove`.
	pub kind: String,
	pub by: String,
	pub before: Option<Value>,
	pub after: Option<Value>,
}

pub async fn insert_change(conn: &mut SqliteConnection, brand: &BrandId, change: &ChangeRecord, by_user: Option<Uuid>) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO pricing_changes (id, brand_id, at, kind, changed_by, changed_by_user, before, after) \
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
	)
	.bind(change.id)
	.bind(brand.as_str())
	.bind(to_db(change.at))
	.bind(&change.kind)
	.bind(&change.by)
	.bind(by_user)
	.bind(change.before.as_ref().map(Json))
	.bind(change.after.as_ref().map(Json))
	.execute(&mut *conn)
	.await
	.wrap_err("journaling a brand's pricing change")?;
	Ok(())
}

/// A brand's last `limit` changes, newest first; two in the same microsecond by insertion order.
pub async fn changes(conn: &mut SqliteConnection, brand: &BrandId, limit: u32) -> eyre::Result<Vec<ChangeRecord>> {
	type ChangeRow = (Uuid, i64, String, String, Option<Json<Value>>, Option<Json<Value>>);
	let rows: Vec<ChangeRow> = sqlx::query_as(
		"SELECT id, at, kind, changed_by, before, after FROM pricing_changes WHERE brand_id = $1 \
		 ORDER BY at DESC, rowid DESC LIMIT $2",
	)
	.bind(brand.as_str())
	.bind(i64::from(limit))
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading a brand's pricing history")?;
	rows.into_iter()
		.map(|(id, at, kind, by, before, after)| {
			Ok(ChangeRecord {
				id,
				at: from_db(at)?,
				kind,
				by,
				before: before.map(|Json(v)| v),
				after: after.map(|Json(v)| v),
			})
		})
		.collect()
}
